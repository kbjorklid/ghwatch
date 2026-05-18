use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ghwatch::app::{App, AppMode};
use ghwatch::domain::attention::AttentionState;
use ghwatch::domain::ports::{GithubProvider, StateRepository};
use ghwatch::domain::pr::{
    CIStatus, CheckRun, MergeableStatus, PRStatus, PullRequest, RateLimitStatus, ReviewStatus,
    TimelineEvent,
};
use mockall::{mock, predicate::*};
use ratatui::backend::TestBackend;
use std::sync::Arc;

mock! {
    pub GithubProvider {}
    #[async_trait]
    impl GithubProvider for GithubProvider {
        async fn fetch_prs_by_query(&self, query: &str, limit: Option<u32>) -> anyhow::Result<Vec<PullRequest>>;
        async fn fetch_pr_details(&self, repo: &str, pr_number: u32) -> anyhow::Result<PullRequest>;
        async fn fetch_check_runs(&self, repo: &str, ref_: &str) -> anyhow::Result<Vec<CheckRun>>;
        async fn fetch_timeline(&self, repo: &str, pr_number: u32) -> anyhow::Result<Vec<TimelineEvent>>;
        async fn fetch_rate_limit(&self) -> anyhow::Result<RateLimitStatus>;
        async fn fetch_current_user(&self) -> anyhow::Result<String>;
        async fn open_pr_in_browser(&self, url: &str) -> anyhow::Result<()>;
    }
}

mock! {
    pub StateRepository {}
    impl StateRepository for StateRepository {
        fn load_state(&self) -> anyhow::Result<Vec<PullRequest>>;
        fn save_state(&self, prs: &[PullRequest]) -> anyhow::Result<()>;
        fn load_archive(&self) -> anyhow::Result<Vec<PullRequest>>;
        fn save_archive(&self, prs: &[PullRequest]) -> anyhow::Result<()>;
        fn archive_pr(&self, pr: PullRequest) -> anyhow::Result<()>;
        fn try_acquire_poll_lease(&self, interval: std::time::Duration) -> anyhow::Result<bool>;
    }
}

fn create_test_pr(id: &str, number: u32) -> PullRequest {
    PullRequest {
        id: id.to_string(),
        number,
        title: format!("PR #{number}"),
        author: "alice".to_string(),
        repo: "org/repo".to_string(),
        status: PRStatus::Open,
        created_at: "2024-05-01T10:00:00Z".to_string(),
        updated_at: "2024-05-01T10:00:00Z".to_string(),
        additions: 10,
        deletions: 5,
        review_status: ReviewStatus::Pending,
        comment_count: 0,
        unresolved_count: 0,
        total_resolvable_count: 0,
        conversational_count: 0,
        ci_status: CIStatus::Passing,
        mergeable: MergeableStatus::Unknown,
        head_ref: "sha123".to_string(),
        body: "Body".to_string(),
        url: String::new(),
        requested_reviewers: vec![],
        reviewers: vec![],
        is_draft: false,
        matched_queries: Vec::new(),
        last_seen_at: None,
        last_seen_unresolved_count: 0,
        last_seen_total_resolvable_count: 0,
        last_seen_conversational_count: 0,
        attention_state: AttentionState::default(),
    }
}

#[tokio::test]
async fn test_navigation_and_mark_as_read() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let pr2 = create_test_pr("2", 2);
    let prs = vec![pr1.clone(), pr2.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir = std::env::temp_dir().join(format!("ghwatch-test-nav-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    assert_eq!(app.pr_list.selected_index(), 0);
    assert_eq!(app.mode, AppMode::Normal);

    // Move down
    let key_down = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_down).await;
    assert_eq!(app.pr_list.selected_index(), 1);

    // Mark as read
    let key_m = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_m).await;

    assert!(app.pr_list.items()[1].last_seen_at.is_some());
    assert_eq!(app.pr_list.items()[1].last_seen_unresolved_count, 0);

    // Update PR with comments
    let mut pr2_updated = app.pr_list.items()[1].clone();
    pr2_updated.unresolved_count = 2;
    pr2_updated.total_resolvable_count = 2;
    pr2_updated.conversational_count = 1;
    app.pr_list.set_prs(vec![app.pr_list.items()[0].clone(), pr2_updated]);

    // Mark as read again
    ghwatch::input::handle_key(&mut app, key_m).await;
    assert_eq!(app.pr_list.items()[1].last_seen_unresolved_count, 2);
    assert_eq!(app.pr_list.items()[1].last_seen_total_resolvable_count, 2);
    assert_eq!(app.pr_list.items()[1].last_seen_conversational_count, 1);

    // Sync from GitHub
    let mut pr2_github = pr2.clone();
    pr2_github.unresolved_count = 3; // One more new unresolved
    pr2_github.total_resolvable_count = 3;
    pr2_github.conversational_count = 2;
    pr2_github.updated_at = "2024-05-01T12:00:00Z".to_string();

    app.merge_prs(vec![pr1.clone(), pr2_github], "test").await;

    let pr2_final = app.pr_list.items().iter().find(|p| p.id == "2").unwrap();
    assert_eq!(pr2_final.last_seen_unresolved_count, 2);
    assert_eq!(pr2_final.last_seen_total_resolvable_count, 2);
    assert_eq!(pr2_final.last_seen_conversational_count, 1);
}

#[tokio::test]
async fn test_search_filtering() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let mut pr2 = create_test_pr("2", 2);
    pr2.title = "Search Me".to_string();
    let prs = vec![pr1.clone(), pr2.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    let temp_dir = std::env::temp_dir().join(format!("ghwatch-test-search-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    // Enter search mode
    let key_slash = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_slash).await;
    assert_eq!(app.mode, AppMode::Search);

    // Type search query
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty()))
        .await;
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('e'), KeyModifiers::empty()))
        .await;
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()))
        .await;

    assert_eq!(app.input_buffer, "sea");

    // Verify filtering
    let filtered = ghwatch::ui::search::filter_prs(app.pr_list.items(), &app.input_buffer);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "2");
}

#[tokio::test]
async fn test_sorting() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let mut pr2 = create_test_pr("2", 2);
    pr2.updated_at = "2024-05-02T10:00:00Z".to_string();
    let prs = vec![pr1.clone(), pr2.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    let temp_dir = std::env::temp_dir().join(format!("ghwatch-test-sort-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    // Explicitly sort
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty()))
        .await; // Sort mode Created
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty()))
        .await; // Sort mode Priority
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty()))
        .await; // Sort mode Repo
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty()))
        .await; // Back to Updated

    // Default sort is Updated (descending)
    assert_eq!(app.pr_list.items()[0].id, "2");
    assert_eq!(app.pr_list.items()[1].id, "1");

    // Change sort to Created
    let key_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_s).await;
    // PRs both have same created_at in create_test_pr, so order might be stable or not depending on sort implementation
    // But let's check it changed from Updated
    assert_eq!(app.sort_mode, ghwatch::app::SortMode::Created);
}

#[tokio::test]
async fn test_priority_sorting() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1); // Passing CI
    let mut pr2 = create_test_pr("2", 2); // Failing CI -> Needs Attention
    pr2.ci_status = CIStatus::Failing;

    let prs = vec![pr1.clone(), pr2.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-priority-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    app.config.current_user = "alice".to_string();

    // Cycle to Priority sort
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty()))
        .await; // Created
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty()))
        .await; // Priority

    assert_eq!(app.sort_mode, ghwatch::app::SortMode::Priority);

    // PR 2 should be first because it needs attention
    assert_eq!(app.pr_list.items()[0].id, "2");
    assert_eq!(app.pr_list.items()[1].id, "1");
}

#[tokio::test]
async fn test_grouping_cycle() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    state_repo.expect_load_state().returning(|| Ok(vec![]));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    let temp_dir = std::env::temp_dir().join(format!("ghwatch-test-group-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    assert_eq!(app.config.group_by, ghwatch::config::GroupMode::None);

    // Cycle group mode (Ctrl+g)
    let key_ctrl_g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
    ghwatch::input::handle_key(&mut app, key_ctrl_g).await;
    assert_eq!(app.config.group_by, ghwatch::config::GroupMode::Repo);

    ghwatch::input::handle_key(&mut app, key_ctrl_g).await;
    assert_eq!(app.config.group_by, ghwatch::config::GroupMode::Author);
}

#[tokio::test]
async fn test_app_modes() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    state_repo.expect_load_state().returning(|| Ok(vec![]));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    let temp_dir = std::env::temp_dir().join(format!("ghwatch-test-modes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    // Help mode
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()))
        .await;
    assert_eq!(app.mode, AppMode::Help);
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())).await;
    assert_eq!(app.mode, AppMode::Normal);

    // Settings mode via Left arrow
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Left, KeyModifiers::empty())).await;
    assert_eq!(app.mode, AppMode::Settings);
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())).await;
    assert_eq!(app.mode, AppMode::Normal);

    // Archive mode via Right arrow
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::empty()))
        .await;
    assert_eq!(app.mode, AppMode::Archive);
}

#[tokio::test]
async fn test_manual_follow() {
    let mut github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr = create_test_pr("1", 1);

    state_repo.expect_load_state().returning(|| Ok(vec![]));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    github
        .expect_fetch_pr_details()
        .with(eq("org/repo"), eq(1))
        .returning(move |_, _| Ok(pr.clone()));

    let temp_dir = std::env::temp_dir().join(format!("ghwatch-test-follow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    // Enter follow mode
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty()))
        .await;
    assert_eq!(app.mode, AppMode::Follow);

    // Type shorthand
    for c in "org/repo#1".chars() {
        ghwatch::input::handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()),
        )
        .await;
    }

    // Press Enter
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()))
        .await;
    assert_eq!(app.mode, AppMode::Normal);

    // Wait for fetch and process the event
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    if let Ok(ghwatch::ui::events::AppEvent::PrsUpdated { prs, query_name }) =
        app.event_rx.try_recv()
    {
        app.merge_prs(prs, &query_name).await;
    }

    assert_eq!(app.pr_list.items().len(), 1);
    assert_eq!(app.pr_list.items()[0].id, "1");
}

#[tokio::test]
async fn test_archiving() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_archive_pr().returning(|_| Ok(()));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-archive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    assert_eq!(app.pr_list.items().len(), 1);

    // Archive
    let key_u = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_u).await;

    assert_eq!(app.pr_list.items().len(), 0);
}

#[tokio::test]
async fn test_archived_pr_not_readded_by_poll() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_archive_pr().returning(|_| Ok(()));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-archive-no-readd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    app.is_first_sync = false;

    // Archive the PR
    let key_u = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_u).await;
    assert_eq!(app.pr_list.items().len(), 0);
    assert_eq!(app.archive_list.items().len(), 1);

    // Simulate a poll returning the same PR (still matches the query on GitHub)
    app.merge_prs(vec![pr1.clone()], "my-query").await;

    assert_eq!(app.pr_list.items().len(), 0, "Archived PR must not reappear in active list");
    assert_eq!(app.archive_list.items().len(), 1, "Archive should still contain the PR");
}

#[tokio::test]
async fn test_archived_pr_removed_from_active_list_on_poll() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    let archived_pr = pr1.clone();
    state_repo.expect_load_archive().returning(move || Ok(vec![archived_pr.clone()]));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-archive-cleanup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    app.is_first_sync = false;

    // Inconsistent state: PR is in both pr_list (from load_state) and archive_list.
    assert_eq!(app.pr_list.items().len(), 1);
    assert_eq!(app.archive_list.items().len(), 1);

    // A poll arrives - inconsistency should heal.
    app.merge_prs(vec![pr1.clone()], "my-query").await;

    assert_eq!(app.pr_list.items().len(), 0, "Archived PR must be removed from active list");
}

#[tokio::test]
async fn test_follow_from_archive() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let archived = pr1.clone();

    state_repo.expect_load_state().returning(|| Ok(vec![]));
    state_repo.expect_load_archive().returning(move || Ok(vec![archived.clone()]));
    state_repo.expect_save_archive().returning(|_| Ok(()));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir = std::env::temp_dir()
        .join(format!("ghwatch-test-follow-from-archive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    // Switch to archive view
    let key_l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_l).await;
    assert_eq!(app.mode, AppMode::Archive);
    assert_eq!(app.archive_list.items().len(), 1);
    assert_eq!(app.pr_list.items().len(), 0);

    // Press 'f' to follow from archive
    let key_f = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_f).await;

    assert_eq!(app.archive_list.items().len(), 0, "PR should be gone from archive");
    assert_eq!(app.pr_list.items().len(), 1, "PR should be in active list");
    assert!(
        app.pr_list.items()[0].matched_queries.iter().any(|q| q == "manual"),
        "Followed PR should carry the 'manual' attribution so it survives the next poll"
    );
}

#[tokio::test]
async fn test_detail_fetching_on_navigation() {
    let mut github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let pr2 = create_test_pr("2", 2);
    let prs = vec![pr1.clone(), pr2.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    // Expect detail fetch for PR 2 when navigating down
    github
        .expect_fetch_pr_details()
        .with(eq("org/repo".to_string()), eq(2))
        .returning(move |_, _| Ok(pr2.clone()));
    github
        .expect_fetch_check_runs()
        .with(eq("org/repo".to_string()), always())
        .returning(|_, _| Ok(vec![]));
    github
        .expect_fetch_timeline()
        .with(eq("org/repo".to_string()), eq(2))
        .returning(|_, _| Ok(vec![]));

    // Also expect initial fetch for PR 1 since it's selected at start
    github
        .expect_fetch_pr_details()
        .with(eq("org/repo".to_string()), eq(1))
        .returning(move |_, _| Ok(pr1.clone()));
    github
        .expect_fetch_timeline()
        .with(eq("org/repo".to_string()), eq(1))
        .returning(|_, _| Ok(vec![]));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-details-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    // Move down to PR 2
    let key_down = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_down).await;

    // Wait a bit for tokio::spawn tasks to finish
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_attention_ci_failed_fires_on_transition() {
    // CI transitions from Passing to Failing → CiFailed reason should be added
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.author = "testuser".to_string();
    pr1.ci_status = CIStatus::Passing;
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-attn-ci-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();
    app.config.current_user = "testuser".to_string();

    // Poll arrives: same PR but CI now failing
    let mut pr1_failing = pr1.clone();
    pr1_failing.ci_status = CIStatus::Failing;
    pr1_failing.updated_at = "2024-05-01T11:00:00Z".to_string();

    app.is_first_sync = false;
    app.merge_prs(vec![pr1_failing], "test").await;

    use ghwatch::domain::attention::TriggerReason;
    let pr = app.pr_list.items().iter().find(|p| p.id == "1").unwrap();
    assert!(
        pr.attention_state.active_reasons.contains(&TriggerReason::CiFailed),
        "CiFailed should be in active_reasons after CI transitions to Failing"
    );
}

#[tokio::test]
async fn test_attention_mentioned_fires_on_timeline_loaded() {
    // PR arrives, then timeline loads with a comment mentioning @testuser → Mentioned fires
    use ghwatch::domain::attention::TriggerReason;
    use ghwatch::ui::events::AppEvent;

    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.author = "otheruser".to_string();
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-attn-mention-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();
    app.config.current_user = "testuser".to_string();

    // Timeline arrives with a comment mentioning @testuser
    let events = vec![TimelineEvent {
        id: "evt1".to_string(),
        event_type: "IssueComment".to_string(),
        actor: "otheruser".to_string(),
        created_at: "2024-05-01T12:00:00Z".to_string(),
        content: Some("Hey @testuser please review".to_string()),
        reviewer_login: None,
    }];

    let event = AppEvent::TimelineLoaded { repo: "org/repo".to_string(), pr_number: 1, events };
    app.handle_app_event(event).await;

    let pr = app.pr_list.items().iter().find(|p| p.id == "1").unwrap();
    assert!(
        pr.attention_state.active_reasons.contains(&TriggerReason::Mentioned),
        "Mentioned should be in active_reasons after timeline mentions @testuser"
    );
}

#[tokio::test]
async fn test_attention_mark_seen_clears_active_reasons() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};

    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    // Pre-seed an active reason
    pr1.attention_state = AttentionState {
        active_reasons: std::iter::once(TriggerReason::CiFailed).collect(),
        last_seen_at: None,
        last_comment_at: None,
    };
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-attn-markseen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    assert!(
        app.pr_list.items()[0].attention_state.active_reasons.contains(&TriggerReason::CiFailed),
        "CiFailed should be active before mark-as-seen"
    );

    let key_m = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_m).await;

    let pr = &app.pr_list.items()[0];
    assert!(
        pr.attention_state.active_reasons.is_empty(),
        "active_reasons should be empty after mark-as-seen"
    );
    assert!(pr.attention_state.last_seen_at.is_some(), "last_seen_at should be set");
}

#[tokio::test]
async fn test_attention_archive_clears_active_reasons() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};

    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.attention_state = AttentionState {
        active_reasons: std::iter::once(TriggerReason::ReviewRequested).collect(),
        last_seen_at: None,
        last_comment_at: None,
    };
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_archive_pr().returning(|_| Ok(()));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-attn-archive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    assert!(
        app.pr_list.items()[0]
            .attention_state
            .active_reasons
            .contains(&TriggerReason::ReviewRequested),
        "ReviewRequested should be active before archive"
    );

    // Press 'u' to archive
    let key_u = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_u).await;

    // PR should now be in archive_list with cleared attention state
    assert_eq!(app.pr_list.items().len(), 0, "PR should be removed from active list");
    let archived = &app.archive_list.items()[0];
    assert!(
        archived.attention_state.active_reasons.is_empty(),
        "active_reasons should be empty after archiving"
    );
}

#[tokio::test]
async fn test_attention_state_field_exists_and_defaults() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let prs = vec![pr1];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-attn-field-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let app = App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
        .unwrap();

    let pr = &app.pr_list.items()[0];
    assert!(pr.attention_state.active_reasons.is_empty());
    assert!(pr.attention_state.last_seen_at.is_none());
}

#[tokio::test]
async fn test_delete_from_archive() {
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);

    state_repo.expect_load_state().returning(|| Ok(vec![]));
    state_repo.expect_load_archive().returning(move || Ok(vec![pr1.clone()]));
    state_repo.expect_save_archive().with(eq(vec![])).returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-delete-archive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    // Switch to archive mode via Right arrow
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::empty()))
        .await;
    assert_eq!(app.mode, AppMode::Archive);
    assert_eq!(app.archive_list.items().len(), 1);

    // Press 'd' to delete
    let key_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_d).await;

    assert_eq!(app.archive_list.items().len(), 0);
}

#[tokio::test]
async fn test_open_in_browser_marks_seen_when_configured() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};
    use std::collections::HashSet;

    let mut github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.url = "https://github.com/org/repo/pull/1".to_string();
    pr1.attention_state = AttentionState {
        active_reasons: HashSet::from([TriggerReason::CiFailed]),
        last_seen_at: None,
        last_comment_at: None,
    };
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));
    github.expect_open_pr_in_browser().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-open-marks-seen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    app.config.attention.open_in_browser_marks_seen = true;

    assert!(
        app.pr_list.items()[0].attention_state.active_reasons.contains(&TriggerReason::CiFailed),
        "PR should have CiFailed before opening"
    );

    // Press 'o' to open in browser
    let key_o = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_o).await;

    assert!(
        app.pr_list.items()[0].attention_state.active_reasons.is_empty(),
        "Attention state should be cleared after open_in_browser with marks_seen config enabled"
    );
    assert!(
        app.pr_list.items()[0].attention_state.last_seen_at.is_some(),
        "last_seen_at should be set after open_in_browser marks seen"
    );
}

#[tokio::test]
async fn test_open_in_browser_does_not_mark_seen_when_disabled() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};
    use std::collections::HashSet;

    let mut github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.url = "https://github.com/org/repo/pull/1".to_string();
    pr1.attention_state = AttentionState {
        active_reasons: HashSet::from([TriggerReason::CiFailed]),
        last_seen_at: None,
        last_comment_at: None,
    };
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    github.expect_open_pr_in_browser().returning(|_| Ok(()));

    let temp_dir = std::env::temp_dir()
        .join(format!("ghwatch-test-open-no-marks-seen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    // open_in_browser_marks_seen defaults to false
    assert!(!app.config.attention.open_in_browser_marks_seen);

    let key_o = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_o).await;

    assert!(
        app.pr_list.items()[0].attention_state.active_reasons.contains(&TriggerReason::CiFailed),
        "Attention state should NOT be cleared when open_in_browser_marks_seen is false"
    );
}

#[tokio::test]
async fn test_converted_to_draft_clears_review_requested() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};
    use std::collections::HashSet;

    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.author = "otheruser".to_string();
    pr1.is_draft = false;
    pr1.attention_state = AttentionState {
        active_reasons: HashSet::from([TriggerReason::ReviewRequested]),
        last_seen_at: None,
        last_comment_at: None,
    };
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-draft-attn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();
    app.config.current_user = "testuser".to_string();

    assert!(
        app.pr_list.items()[0]
            .attention_state
            .active_reasons
            .contains(&TriggerReason::ReviewRequested),
        "ReviewRequested should be active before conversion to draft"
    );

    // Poll arrives: same PR but now is_draft=true
    let mut pr1_draft = pr1.clone();
    pr1_draft.is_draft = true;
    pr1_draft.updated_at = "2024-05-01T11:00:00Z".to_string();

    app.is_first_sync = false;
    app.merge_prs(vec![pr1_draft], "test").await;

    let pr = app.pr_list.items().iter().find(|p| p.id == "1").unwrap();
    assert!(
        !pr.attention_state.active_reasons.contains(&TriggerReason::ReviewRequested),
        "ReviewRequested should be cleared after PR converts to draft"
    );
}

// Test 112: Mark-as-seen on browser open occurs regardless of whether browser opened successfully
#[tokio::test]
async fn test_open_in_browser_marks_seen_even_when_browser_fails() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};
    use std::collections::HashSet;

    let mut github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.url = "https://github.com/org/repo/pull/1".to_string();
    pr1.attention_state = AttentionState {
        active_reasons: HashSet::from([TriggerReason::Approved]),
        last_seen_at: None,
        last_comment_at: None,
    };
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));
    github
        .expect_open_pr_in_browser()
        .returning(|_| Err(anyhow::anyhow!("browser failed to open")));

    let temp_dir = std::env::temp_dir()
        .join(format!("ghwatch-test-browser-fail-marks-seen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    app.config.attention.open_in_browser_marks_seen = true;

    assert!(
        app.pr_list.items()[0].attention_state.active_reasons.contains(&TriggerReason::Approved),
        "PR should have Approved before opening"
    );

    let key_o = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_o).await;

    assert!(
        app.pr_list.items()[0].attention_state.active_reasons.is_empty(),
        "Attention state should be cleared even when browser fails to open"
    );
    assert!(
        app.pr_list.items()[0].attention_state.last_seen_at.is_some(),
        "last_seen_at should be set even when browser fails to open"
    );
}

// Test 114: Marking as seen does not reset the comment delta display
#[tokio::test]
async fn test_mark_seen_does_not_reset_comment_delta() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};
    use std::collections::HashSet;

    let mut github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.unresolved_count = 2;
    pr1.total_resolvable_count = 2;
    pr1.conversational_count = 3;
    pr1.attention_state = AttentionState {
        active_reasons: HashSet::from([TriggerReason::CiFailed]),
        last_seen_at: None,
        last_comment_at: None,
    };
    let prs = vec![pr1.clone()];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));
    github.expect_open_pr_in_browser().returning(|_| Ok(()));

    let temp_dir = std::env::temp_dir()
        .join(format!("ghwatch-test-mark-seen-comment-delta-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    let key_m = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_m).await;

    let pr = &app.pr_list.items()[0];
    assert!(
        pr.attention_state.active_reasons.is_empty(),
        "Active reasons should be cleared after mark-as-seen"
    );
    assert_eq!(pr.unresolved_count, 2, "unresolved_count must not be zeroed by mark-as-seen");
    assert_eq!(
        pr.conversational_count, 3,
        "conversational_count must not be zeroed by mark-as-seen"
    );
}

// Test 115: Active reasons survive application restart
#[tokio::test]
async fn test_active_reasons_survive_restart() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};
    use std::collections::HashSet;

    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.attention_state = AttentionState {
        active_reasons: HashSet::from([TriggerReason::CiFailed, TriggerReason::Approved]),
        last_seen_at: None,
        last_comment_at: None,
    };
    let prs = vec![pr1];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    let temp_dir = std::env::temp_dir()
        .join(format!("ghwatch-test-reasons-survive-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let app = App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
        .unwrap();

    let pr = &app.pr_list.items()[0];
    assert!(
        pr.attention_state.active_reasons.contains(&TriggerReason::CiFailed),
        "CiFailed should survive restart"
    );
    assert!(
        pr.attention_state.active_reasons.contains(&TriggerReason::Approved),
        "Approved should survive restart"
    );
}

// Test 116: last_seen_at survives application restart
#[tokio::test]
async fn test_last_seen_at_survives_restart() {
    use chrono::DateTime;
    use ghwatch::domain::attention::AttentionState;
    use std::collections::HashSet;

    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let seen_time: DateTime<chrono::Utc> = "2024-05-01T10:30:00Z".parse().unwrap();
    let mut pr1 = create_test_pr("1", 1);
    pr1.attention_state = AttentionState {
        active_reasons: HashSet::default(),
        last_seen_at: Some(seen_time),
        last_comment_at: None,
    };
    let prs = vec![pr1];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    let temp_dir = std::env::temp_dir()
        .join(format!("ghwatch-test-last-seen-survive-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let app = App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
        .unwrap();

    let pr = &app.pr_list.items()[0];
    assert_eq!(
        pr.attention_state.last_seen_at,
        Some(seen_time),
        "last_seen_at should survive restart"
    );
}

// Test 117: last_comment_at survives application restart
#[tokio::test]
async fn test_last_comment_at_survives_restart() {
    use chrono::DateTime;
    use ghwatch::domain::attention::AttentionState;
    use std::collections::HashSet;

    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let comment_time: DateTime<chrono::Utc> = "2024-05-01T10:45:00Z".parse().unwrap();
    let mut pr1 = create_test_pr("1", 1);
    pr1.attention_state = AttentionState {
        active_reasons: HashSet::default(),
        last_seen_at: None,
        last_comment_at: Some(comment_time),
    };
    let prs = vec![pr1];

    state_repo.expect_load_state().returning(move || Ok(prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));

    let temp_dir = std::env::temp_dir()
        .join(format!("ghwatch-test-last-comment-survive-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let app = App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
        .unwrap();

    let pr = &app.pr_list.items()[0];
    assert_eq!(
        pr.attention_state.last_comment_at,
        Some(comment_time),
        "last_comment_at should survive restart so quiet period is preserved"
    );
}

// Test 118: PRs loaded from saved state without attention fields are treated as first appearance
#[tokio::test]
async fn test_saved_state_without_attention_fields_treated_as_first_appearance() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};
    use ghwatch::domain::pr::CIStatus;

    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    // Saved state PR: alice's PR with CI failing, but no attention fields (migration scenario)
    let mut saved_pr = create_test_pr("20", 20);
    saved_pr.author = "testuser".to_string();
    saved_pr.ci_status = CIStatus::Failing;
    saved_pr.attention_state = AttentionState::default(); // no attention fields
    let saved_prs = vec![saved_pr.clone()];

    state_repo.expect_load_state().returning(move || Ok(saved_prs.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir = std::env::temp_dir()
        .join(format!("ghwatch-test-no-attn-first-appearance-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    app.config.current_user = "testuser".to_string();

    // First poll: GitHub returns the same PR with CI still failing — no data change
    let mut github_pr = saved_pr.clone();
    github_pr.ci_status = CIStatus::Failing;

    // is_first_sync is still true here; merge_prs should treat the PR as first appearance
    app.merge_prs(vec![github_pr], "test").await;

    let pr = app.pr_list.items().iter().find(|p| p.id == "20").unwrap();
    assert!(
        pr.attention_state.active_reasons.contains(&TriggerReason::CiFailed),
        "CiFailed should fire retroactively for PR with no prior attention state"
    );
}

#[tokio::test]
async fn test_restrictive_query_drops_unmatched_prs_after_initial_sync() {
    // Persisted state has 3 PRs (legacy: no matched_queries).
    // The query now only returns PR 1. After InitialSyncDone, PRs 2 and 3
    // — never attributed to any query — should be dropped from the list.
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let pr1 = create_test_pr("1", 1);
    let pr2 = create_test_pr("2", 2);
    let pr3 = create_test_pr("3", 3);
    let persisted = vec![pr1.clone(), pr2, pr3];

    state_repo.expect_load_state().returning(move || Ok(persisted.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-restrictive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    assert_eq!(app.pr_list.items().len(), 3, "all 3 PRs loaded from persisted state");

    app.merge_prs(vec![pr1.clone()], "main").await;
    assert_eq!(
        app.pr_list.items().len(),
        3,
        "while is_first_sync is true, unattributed PRs stay (other queries may still match them)"
    );

    app.handle_app_event(ghwatch::ui::events::AppEvent::InitialSyncDone).await;

    assert_eq!(
        app.pr_list.items().len(),
        1,
        "after InitialSyncDone, PRs not matched by any query are dropped"
    );
    assert_eq!(app.pr_list.items()[0].id, "1");
}

#[tokio::test]
async fn test_pr_dropped_when_query_stops_matching_after_initial_sync() {
    // PR 1 is attributed to query "main". After initial sync completes,
    // if query "main" no longer returns PR 1, it should be dropped.
    let github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr1 = create_test_pr("1", 1);
    pr1.matched_queries = vec!["main".to_string()];

    let persisted = vec![pr1];
    state_repo.expect_load_state().returning(move || Ok(persisted.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));

    let temp_dir = std::env::temp_dir().join(format!("ghwatch-test-drop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();

    app.handle_app_event(ghwatch::ui::events::AppEvent::InitialSyncDone).await;
    assert_eq!(app.pr_list.items().len(), 1);

    // Query polls and returns empty — PR 1 had "main" attribution but is no longer matched.
    app.merge_prs(vec![], "main").await;

    assert_eq!(
        app.pr_list.items().len(),
        0,
        "PR dropped once it has no remaining query attribution"
    );
}

// Regression: after marking a PR as seen, a subsequent poll that detects a new
// PR (but returns the same data for the marked PR) must NOT bring the "needs
// attention" reasons back on the marked PR.
#[tokio::test]
async fn test_mark_seen_survives_poll_with_new_pr_added() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};
    use std::collections::HashSet;

    let mut github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    // PR A is the user's own PR with failing CI, attribution to query "main".
    let mut pr_a = create_test_pr("A", 100);
    pr_a.author = "alice".to_string();
    pr_a.ci_status = CIStatus::Failing;
    pr_a.matched_queries = vec!["main".to_string()];
    pr_a.attention_state = AttentionState {
        active_reasons: HashSet::from([TriggerReason::CiFailed]),
        last_seen_at: None,
        last_comment_at: None,
    };

    let persisted = vec![pr_a.clone()];
    state_repo.expect_load_state().returning(move || Ok(persisted.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));
    github
        .expect_fetch_pr_details()
        .returning(|_, _| Err(anyhow::anyhow!("not used in this test")));
    github.expect_fetch_timeline().returning(|_, _| Ok(vec![]));
    github.expect_fetch_check_runs().returning(|_, _| Ok(vec![]));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-mark-seen-new-pr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();
    app.config.current_user = "alice".to_string();

    // Finish initial sync so subsequent merges are treated as post-startup.
    app.handle_app_event(ghwatch::ui::events::AppEvent::InitialSyncDone).await;

    // User marks PR A as seen — active_reasons should clear, last_seen_at set.
    let key_m = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_m).await;
    {
        let pr = app.pr_list.items().iter().find(|p| p.id == "A").unwrap();
        assert!(
            pr.attention_state.active_reasons.is_empty(),
            "active_reasons should be empty immediately after mark-as-seen"
        );
        assert!(
            pr.attention_state.last_seen_at.is_some(),
            "last_seen_at should be set after mark-as-seen"
        );
    }

    // Poll returns PR A unchanged plus a brand-new PR B in the same query.
    let pr_a_unchanged = {
        let pr = app.pr_list.items().iter().find(|p| p.id == "A").unwrap().clone();
        // Simulate what the API would return: no attention_state, no last_seen tracking.
        PullRequest {
            attention_state: AttentionState::default(),
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            matched_queries: Vec::new(),
            ..pr
        }
    };
    let mut pr_b = create_test_pr("B", 200);
    pr_b.author = "bob".to_string();

    app.merge_prs(vec![pr_a_unchanged, pr_b], "main").await;

    let pr_a_after = app.pr_list.items().iter().find(|p| p.id == "A").unwrap();
    assert!(
        pr_a_after.attention_state.active_reasons.is_empty(),
        "active_reasons must NOT come back after a poll that adds a different new PR; \
         got {:?}",
        pr_a_after.attention_state.active_reasons
    );
}

// Regression: when a PR temporarily disappears from query results and later
// reappears (e.g., due to GitHub search eventual consistency), the existing
// mark-as-seen state must NOT be reset by treating the PR as brand-new.
#[tokio::test]
async fn test_mark_seen_survives_pr_disappear_and_reappear() {
    use ghwatch::domain::attention::{AttentionState, TriggerReason};
    use std::collections::HashSet;

    let mut github = MockGithubProvider::new();
    let mut state_repo = MockStateRepository::new();

    let mut pr_a = create_test_pr("A", 100);
    pr_a.author = "alice".to_string();
    pr_a.ci_status = CIStatus::Failing;
    pr_a.matched_queries = vec!["main".to_string()];
    pr_a.attention_state = AttentionState {
        active_reasons: HashSet::from([TriggerReason::CiFailed]),
        last_seen_at: None,
        last_comment_at: None,
    };

    let persisted = vec![pr_a.clone()];
    state_repo.expect_load_state().returning(move || Ok(persisted.clone()));
    state_repo.expect_load_archive().returning(|| Ok(vec![]));
    state_repo.expect_save_state().returning(|_| Ok(()));
    github
        .expect_fetch_pr_details()
        .returning(|_, _| Err(anyhow::anyhow!("not used in this test")));
    github.expect_fetch_timeline().returning(|_, _| Ok(vec![]));
    github.expect_fetch_check_runs().returning(|_, _| Ok(vec![]));

    let temp_dir =
        std::env::temp_dir().join(format!("ghwatch-test-mark-seen-flicker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let backend = TestBackend::new(80, 24);
    let mut app =
        App::with_deps(Arc::new(github), Arc::new(state_repo), &temp_dir, &temp_dir, backend)
            .unwrap();
    app.config.current_user = "alice".to_string();

    app.handle_app_event(ghwatch::ui::events::AppEvent::InitialSyncDone).await;

    let key_m = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_m).await;
    assert!(
        app.pr_list.items()[0].attention_state.active_reasons.is_empty(),
        "active_reasons should be empty after mark-as-seen"
    );

    // PR A vanishes from query results (e.g., transient search eventual consistency).
    app.merge_prs(vec![], "main").await;

    // PR A comes back the very next cycle, still failing CI.
    let mut pr_a_return = pr_a.clone();
    pr_a_return.attention_state = AttentionState::default();
    pr_a_return.matched_queries = Vec::new();
    app.merge_prs(vec![pr_a_return], "main").await;

    let pr_a_after = app
        .pr_list
        .items()
        .iter()
        .find(|p| p.id == "A")
        .expect("PR A should be present after reappearing in the query");
    assert!(
        pr_a_after.attention_state.active_reasons.is_empty(),
        "mark-as-seen must persist across transient drop/reappear; \
         got reasons {:?}",
        pr_a_after.attention_state.active_reasons
    );
}
