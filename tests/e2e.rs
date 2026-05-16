use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ghwatch::app::{App, AppMode};
use ghwatch::domain::ports::{GithubProvider, StateRepository};
use ghwatch::domain::pr::{
    CIStatus, CheckRun, PRStatus, PullRequest, RateLimitStatus, ReviewStatus, TimelineEvent,
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
        head_ref: "sha123".to_string(),
        body: "Body".to_string(),
        url: String::new(),
        requested_reviewers: vec![],
        reviewers: vec![],
        last_seen_at: None,
        last_seen_unresolved_count: 0,
        last_seen_total_resolvable_count: 0,
        last_seen_conversational_count: 0,
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

    app.merge_prs(vec![pr1.clone(), pr2_github], false).await;

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

    // Settings mode
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('S'), KeyModifiers::empty()))
        .await;
    assert_eq!(app.mode, AppMode::Settings);
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())).await;
    assert_eq!(app.mode, AppMode::Normal);

    // Archive mode
    ghwatch::input::handle_key(&mut app, KeyEvent::new(KeyCode::Char('A'), KeyModifiers::empty()))
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
        app.merge_prs(prs, query_name == "detail").await;
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

    // Switch to archive mode
    let key_a = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_a).await;
    assert_eq!(app.mode, AppMode::Archive);
    assert_eq!(app.archive_list.items().len(), 1);

    // Press 'd' to delete
    let key_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
    ghwatch::input::handle_key(&mut app, key_d).await;

    assert_eq!(app.archive_list.items().len(), 0);
}
