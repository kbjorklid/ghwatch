use crate::config::AppConfig;
use crate::domain::attention;
use crate::domain::ports::{GithubProvider, StateRepository};
use crate::domain::pr::{CheckRun, PullRequest, TimelineEvent};
use crate::github::client::GhCliClient;
use crate::polling::worker::PollingWorker;
use crate::storage::get_data_dir;
use crate::storage::local::FileStateRepository;
use crate::ui::events::AppEvent;
use crate::ui::render::Renderer;
use anyhow::Result;
use chrono::Utc;
use crossterm::event;
use ratatui::backend::Backend;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::storage::lock::FileLock;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AppMode {
    Normal,
    Search,
    Follow,
    Settings,
    Archive,
    Help,
    Diagnostic,
    LogDetail,
    AddQueryName,
    AddQuerySearch,
    ConfirmQuery,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SortMode {
    Updated,
    Created,
    Priority,
    Repo,
}

use crate::notify::dispatcher::NotificationDispatcher;

use crate::domain::pr_list::PRList;

#[allow(clippy::struct_excessive_bools, missing_debug_implementations)]
pub struct App<B: Backend>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    pub pr_list: PRList,
    pub archive_list: PRList,
    pub settings_selected_index: usize,
    pub diagnostic_selected_index: usize,
    pub renderer: Renderer<B>,
    pub event_rx: mpsc::Receiver<AppEvent>,
    pub event_tx: mpsc::Sender<AppEvent>,
    pub should_quit: bool,
    pub github: Arc<dyn GithubProvider>,
    pub state_repo: Arc<dyn StateRepository>,
    pub current_checks: Vec<CheckRun>,
    pub current_timeline: Vec<TimelineEvent>,
    pub config: AppConfig,
    pub is_writer: bool,
    _lock: Option<FileLock>,
    pub mode: AppMode,
    pub sort_mode: SortMode,
    pub detail_focused: bool,
    pub detail_scroll: u16,
    pub input_buffer: String,
    pub config_watcher: Option<crate::config::watcher::ConfigWatcher>,
    pub state_watcher: Option<crate::config::watcher::StateWatcher>,
    pub notifier: Box<dyn crate::domain::ports::NotificationService>,
    pub last_refresh: Option<std::time::Instant>,
    pub polling_task: Option<tokio::task::JoinHandle<()>>,
    pub is_first_sync: bool,
    pub error_message: Option<String>,
    pub error_time: Option<std::time::Instant>,
    pub query_name_buffer: String,
    pub query_search_buffer: String,
    pub query_test_results: Option<Vec<PullRequest>>,
    pub query_test_error: Option<String>,
    pub is_testing_query: bool,
    pub pr_timelines: HashMap<String, Vec<TimelineEvent>>,
}

impl App<ratatui::backend::CrosstermBackend<std::io::Stdout>> {
    pub fn new() -> Result<Self> {
        let config_dir =
            crate::storage::get_config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let data_dir = get_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

        let github = Arc::new(GhCliClient::new());
        let state_repo = Arc::new(FileStateRepository::new(&data_dir));

        let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
        Self::with_deps(github, state_repo, &config_dir, &data_dir, backend)
    }
}

impl<B: Backend> App<B>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    pub fn with_deps(
        github: Arc<dyn GithubProvider>,
        state_repo: Arc<dyn StateRepository>,
        config_dir: &std::path::Path,
        data_dir: &std::path::Path,
        backend: B,
    ) -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);
        let config_path = config_dir.join("config.toml");
        let config = AppConfig::load(&config_path).unwrap_or_default();

        let _ = std::fs::create_dir_all(data_dir);
        let lock_path = data_dir.join(".lock");

        let (is_writer, lock) = match FileLock::acquire_exclusive(&lock_path) {
            Ok(lock) => (true, Some(lock)),
            Err(_) => match FileLock::acquire_shared(&lock_path) {
                Ok(lock) => (false, Some(lock)),
                Err(_) => (false, None),
            },
        };

        let prs = state_repo.load_state().unwrap_or_default();
        let archived_prs = state_repo.load_archive().unwrap_or_default();
        let notifier = Box::new(NotificationDispatcher::new(true));

        Ok(Self {
            pr_list: PRList::new(prs),
            archive_list: PRList::new(archived_prs),
            settings_selected_index: 0,
            diagnostic_selected_index: 0,
            renderer: Renderer::new(backend)?,
            event_rx: rx,
            event_tx: tx,
            should_quit: false,
            github,
            state_repo,
            current_checks: Vec::new(),
            current_timeline: Vec::new(),
            config,
            is_writer,
            _lock: lock,
            mode: AppMode::Normal,
            sort_mode: SortMode::Updated,
            detail_focused: false,
            detail_scroll: 0,
            input_buffer: String::new(),
            config_watcher: None,
            state_watcher: None,
            notifier,
            last_refresh: None,
            polling_task: None,
            is_first_sync: true,
            error_message: None,
            error_time: None,
            query_name_buffer: String::new(),
            query_search_buffer: String::new(),
            query_test_results: None,
            query_test_error: None,
            is_testing_query: false,
            pr_timelines: HashMap::new(),
        })
    }
}

impl<B: Backend> App<B>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    pub async fn run(&mut self, _init_renderer: bool) -> Result<()> {
        let config_dir =
            crate::storage::get_config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let config_path = config_dir.join("config.toml");
        if config_path.exists() {
            self.config_watcher =
                crate::config::watcher::ConfigWatcher::new(&config_path, self.event_tx.clone())
                    .ok();
        }

        if self.config.current_user.is_empty() {
            let output = Command::new("gh").args(["api", "user", "--jq", ".login"]).output().await;
            if let Ok(output) = output {
                self.config.current_user =
                    String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }

        if self.is_writer {
            let polling_worker =
                PollingWorker::new(self.config.clone(), self.github.clone(), self.event_tx.clone());
            self.polling_task = Some(tokio::spawn(polling_worker.start()));
        } else {
            let data_dir = get_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let state_path = data_dir.join("state.toml");
            if state_path.exists() {
                self.state_watcher =
                    crate::config::watcher::StateWatcher::new(&state_path, self.event_tx.clone())
                        .ok();
            }
        }

        // Check gh CLI
        let gh_check = Command::new("gh").arg("--version").output().await;
        if gh_check.is_err() {
            let _ =
                self.event_tx.send(AppEvent::Error("gh CLI not found in PATH".to_string())).await;
        }

        let input_tx = self.event_tx.clone();

        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(100)).unwrap() {
                    let e = event::read().unwrap();
                    let _ = input_tx.send(AppEvent::Input(e)).await;
                }
                let _ = input_tx.send(AppEvent::Tick).await;
            }
        });

        while !self.should_quit {
            let (draw_prs, draw_index) = match self.mode {
                AppMode::Search => (
                    crate::ui::search::filter_prs(self.pr_list.items(), &self.input_buffer),
                    self.pr_list.selected_index(),
                ),
                AppMode::Archive => {
                    (self.archive_list.items().to_vec(), self.archive_list.selected_index())
                }
                _ => (self.pr_list.items().to_vec(), self.pr_list.selected_index()),
            };

            self.renderer.draw(&crate::ui::render::DrawContext {
                prs: &draw_prs,
                selected_index: draw_index,
                settings_selected_index: self.settings_selected_index,
                diagnostic_selected_index: self.diagnostic_selected_index,
                checks: &self.current_checks,
                timeline: &self.current_timeline,
                mode: &self.mode,
                detail_focused: self.detail_focused,
                detail_scroll: self.detail_scroll,
                input_buffer: &self.input_buffer,
                config: &self.config,
                last_refresh: self.last_refresh,
                error_message: self.error_message.as_deref(),
                query_name_buffer: &self.query_name_buffer,
                query_search_buffer: &self.query_search_buffer,
                query_test_results: self.query_test_results.as_deref(),
                query_test_error: self.query_test_error.as_deref(),
                is_testing_query: self.is_testing_query,
            })?;

            if let Some(event) = self.event_rx.recv().await {
                self.handle_app_event(event).await;
            }
        }

        Ok(())
    }

    pub async fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Input(e) => crate::input::handle_event(self, e).await,
            AppEvent::Tick => {
                if let Some(t) = self.error_time
                    && t.elapsed() > std::time::Duration::from_secs(10)
                {
                    self.error_message = None;
                    self.error_time = None;
                }
            }
            AppEvent::PrsUpdated { query_name, prs } => {
                self.merge_prs(prs, query_name == "detail").await;
                if self.is_writer {
                    let _ = self.state_repo.save_state(self.pr_list.items());
                }
            }
            AppEvent::CiStatusLoaded { repo, pr_number, checks } => {
                if let Some(selected) = self.pr_list.selected_pr()
                    && selected.repo == repo
                    && selected.number == pr_number
                {
                    self.current_checks = checks;
                }
            }
            AppEvent::TimelineLoaded { repo, pr_number, events } => {
                if let Some(idx) = self
                    .pr_list
                    .items()
                    .iter()
                    .position(|p| p.repo == repo && p.number == pr_number)
                {
                    let pr_id = self.pr_list.items()[idx].id.clone();
                    self.pr_timelines.insert(pr_id, events.clone());

                    let pr = &self.pr_list.items()[idx];
                    let new_attn = attention::evaluate(
                        Some(&pr.attention_state),
                        Some(pr),
                        pr,
                        &events,
                        &self.config.current_user,
                        Utc::now(),
                        &self.config.attention,
                    );
                    self.pr_list.items_mut()[idx].attention_state = new_attn;

                    if self.is_writer {
                        let _ = self.state_repo.save_state(self.pr_list.items());
                    }
                }

                if let Some(selected) = self.pr_list.selected_pr()
                    && selected.repo == repo
                    && selected.number == pr_number
                {
                    self.current_timeline = events;
                }
            }
            AppEvent::PollCycleStarted => {
                self.notifier.clear_cycle();
            }
            AppEvent::InitialSyncDone => {
                self.is_first_sync = false;
            }
            AppEvent::ConfigReloaded(new_config) => {
                self.handle_config_reload(new_config);
            }
            AppEvent::StateReloaded(new_prs) => {
                self.pr_list.set_prs(new_prs);
                self.sort_prs();
            }
            AppEvent::Error(msg) => {
                self.error_message = Some(msg);
                self.error_time = Some(std::time::Instant::now());
            }
            AppEvent::QueryTested(res) => {
                self.is_testing_query = false;
                match res {
                    Ok(prs) => {
                        self.query_test_results = Some(prs);
                        self.query_test_error = None;
                    }
                    Err(e) => {
                        self.query_test_results = None;
                        self.query_test_error = Some(e);
                    }
                }
            }
        }
    }

    pub fn handle_config_reload(&mut self, new_config: AppConfig) {
        self.config = new_config;
        if self.is_writer {
            if let Some(task) = self.polling_task.take() {
                task.abort();
            }
            let polling_worker =
                PollingWorker::new(self.config.clone(), self.github.clone(), self.event_tx.clone());
            self.polling_task = Some(tokio::spawn(polling_worker.start()));
        }
    }

    pub async fn merge_prs(&mut self, new_prs: Vec<PullRequest>, is_detail: bool) {
        if !is_detail {
            self.last_refresh = Some(std::time::Instant::now());
        }

        if is_detail {
            if let Some(new_pr) = new_prs.first()
                && let Some(old_pr) =
                    self.pr_list.items_mut().iter_mut().find(|p| p.id == new_pr.id)
            {
                if !self.is_first_sync {
                    self.notifier.notify_pr_update(old_pr, new_pr);
                }
                let last_seen = old_pr.last_seen_at.clone();
                let seen_unresolved = old_pr.last_seen_unresolved_count;
                let seen_total = old_pr.last_seen_total_resolvable_count;
                let seen_conv = old_pr.last_seen_conversational_count;
                let attn = old_pr.attention_state.clone();
                *old_pr = new_pr.clone();
                old_pr.last_seen_at = last_seen;
                old_pr.last_seen_unresolved_count = seen_unresolved;
                old_pr.last_seen_total_resolvable_count = seen_total;
                old_pr.last_seen_conversational_count = seen_conv;
                old_pr.attention_state = attn;
            }
        } else {
            let mut current_prs = self.pr_list.items().to_vec();
            for new_pr in new_prs {
                if let Some(old_pr) = current_prs.iter_mut().find(|p| p.id == new_pr.id) {
                    let has_changed = old_pr.updated_at != new_pr.updated_at
                        || old_pr.ci_status != new_pr.ci_status
                        || old_pr.review_status != new_pr.review_status
                        || old_pr.comment_count != new_pr.comment_count
                        || old_pr.total_resolvable_count != new_pr.total_resolvable_count
                        || old_pr.unresolved_count != new_pr.unresolved_count;

                    if has_changed {
                        if !self.is_first_sync {
                            self.notifier.notify_pr_update(old_pr, &new_pr);
                        }
                        let timeline: &[TimelineEvent] =
                            self.pr_timelines.get(&new_pr.id).map_or(&[], Vec::as_slice);
                        let new_attn = attention::evaluate(
                            Some(&old_pr.attention_state),
                            Some(old_pr),
                            &new_pr,
                            timeline,
                            &self.config.current_user,
                            Utc::now(),
                            &self.config.attention,
                        );
                        let last_seen = old_pr.last_seen_at.clone();
                        let seen_unresolved = old_pr.last_seen_unresolved_count;
                        let seen_total = old_pr.last_seen_total_resolvable_count;
                        let seen_conv = old_pr.last_seen_conversational_count;
                        *old_pr = new_pr.clone();
                        old_pr.last_seen_at = last_seen;
                        old_pr.last_seen_unresolved_count = seen_unresolved;
                        old_pr.last_seen_total_resolvable_count = seen_total;
                        old_pr.last_seen_conversational_count = seen_conv;
                        old_pr.attention_state = new_attn;
                        self.trigger_details_fetch().await;
                    }
                } else {
                    let new_attn = attention::evaluate(
                        None,
                        None,
                        &new_pr,
                        &[],
                        &self.config.current_user,
                        Utc::now(),
                        &self.config.attention,
                    );
                    if !self.is_first_sync {
                        self.notifier.notify_new_pr(&new_pr);
                    }
                    let mut pr = new_pr;
                    pr.attention_state = new_attn;
                    current_prs.push(pr);
                    self.trigger_details_fetch().await;
                }
            }
            self.pr_list.set_prs(current_prs);
            self.sort_prs();
            self.check_auto_unfollow();
        }
    }

    pub fn check_auto_unfollow(&mut self) {
        if !self.is_writer {
            return;
        }

        let timeout = self.config.unfollow_timeout_mins;
        let mut to_remove = Vec::new();

        for (i, pr) in self.pr_list.items().iter().enumerate() {
            if crate::domain::lifecycle::should_auto_unfollow(pr, timeout) {
                to_remove.push(i);
            }
        }

        // Remove from highest index to lowest to avoid index shifts
        let mut current_prs = self.pr_list.items().to_vec();
        for &i in to_remove.iter().rev() {
            let pr = current_prs.remove(i);
            self.archive_list.insert_at_front(pr.clone());
            let _ = self.state_repo.archive_pr(pr);
        }

        if !to_remove.is_empty() {
            self.pr_list.set_prs(current_prs);
            let _ = self.state_repo.save_state(self.pr_list.items());
        }
    }

    pub fn sort_prs(&mut self) {
        let sort_mode = self.sort_mode;
        let config_user = self.config.current_user.clone();
        let group_mode = self.config.group_by.clone();

        let mut prs = self.pr_list.items().to_vec();
        prs.sort_by(|a, b| {
            // 1. Group sort
            use crate::config::GroupMode;
            let group_cmp = match group_mode {
                GroupMode::None => std::cmp::Ordering::Equal,
                GroupMode::Repo => a.repo.cmp(&b.repo),
                GroupMode::Author => a.author.cmp(&b.author),
                GroupMode::Status => a.status.to_string().cmp(&b.status.to_string()),
                GroupMode::MyVsOther => {
                    let a_mine = a.author == config_user;
                    let b_mine = b.author == config_user;
                    b_mine.cmp(&a_mine) // true comes before false
                }
            };

            if group_cmp != std::cmp::Ordering::Equal {
                return group_cmp;
            }

            // 2. Secondary sort
            match sort_mode {
                SortMode::Updated => b.updated_at.cmp(&a.updated_at),
                SortMode::Created => b.created_at.cmp(&a.created_at),
                SortMode::Repo => a.repo.cmp(&b.repo),
                SortMode::Priority => {
                    let a_att = crate::domain::rules::needs_attention(a, &config_user);
                    let b_att = crate::domain::rules::needs_attention(b, &config_user);
                    b_att.cmp(&a_att).then_with(|| b.updated_at.cmp(&a.updated_at))
                }
            }
        });
        self.pr_list.set_prs(prs);
    }

    pub async fn follow_pr(&mut self, input: &str) {
        let input = input.trim();
        let (repo, number) = if input.starts_with("http") {
            let parts: Vec<&str> = input.split('/').collect();
            if parts.len() >= 7 {
                (format!("{}/{}", parts[3], parts[4]), parts[6].parse::<u32>().ok())
            } else {
                return;
            }
        } else if let Some((repo, num_str)) = input.split_once('#') {
            (repo.trim().to_string(), num_str.trim().parse::<u32>().ok())
        } else {
            return;
        };

        if let Some(number) = number {
            let github = self.github.clone();
            let tx = self.event_tx.clone();
            tokio::spawn(async move {
                match github.fetch_pr_details(&repo, number).await {
                    Ok(pr) => {
                        let _ = tx
                            .send(AppEvent::PrsUpdated {
                                query_name: "manual".to_string(),
                                prs: vec![pr],
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(AppEvent::Error(format!(
                                "Failed to fetch PR {repo}#{number}: {e}"
                            )))
                            .await;
                    }
                }
            });
        }
    }
    pub async fn trigger_details_fetch(&mut self) {
        if let Some(pr) = self.pr_list.selected_pr() {
            let tx = self.event_tx.clone();
            let github = self.github.clone();
            let repo = pr.repo.clone();
            let number = pr.number;

            tokio::spawn(async move {
                if let Ok(full_pr) = github.fetch_pr_details(&repo, number).await {
                    let _ = tx
                        .send(AppEvent::PrsUpdated {
                            query_name: "detail".to_string(),
                            prs: vec![full_pr.clone()],
                        })
                        .await;

                    if !full_pr.head_ref.is_empty()
                        && let Ok(checks) = github.fetch_check_runs(&repo, &full_pr.head_ref).await
                    {
                        let _ = tx
                            .send(AppEvent::CiStatusLoaded {
                                repo: repo.clone(),
                                pr_number: number,
                                checks,
                            })
                            .await;
                    }
                }

                if let Ok(timeline) = github.fetch_timeline(&repo, number).await {
                    let _ = tx
                        .send(AppEvent::TimelineLoaded {
                            repo: repo.clone(),
                            pr_number: number,
                            events: timeline,
                        })
                        .await;
                }
            });
        }
    }

    pub fn copy_to_clipboard(&mut self, text: &str) {
        use arboard::Clipboard;
        match Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(text) {
                    self.error_message = Some(format!("Failed to copy: {e}"));
                } else {
                    self.error_message = Some("Copied to clipboard!".to_string());
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Clipboard error: {e}"));
            }
        }
        self.error_time = Some(std::time::Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::{GithubProvider, NotificationService, StateRepository};
    use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, PullRequest, ReviewStatus};
    use async_trait::async_trait;
    use mockall::mock;
    use std::sync::Arc;

    mock! {
        pub GithubProvider {}
        #[async_trait]
        impl GithubProvider for GithubProvider {
            async fn fetch_prs_by_query(&self, query: &str, limit: Option<u32>) -> anyhow::Result<Vec<PullRequest>>;
            async fn fetch_pr_details(&self, repo: &str, pr_number: u32) -> anyhow::Result<PullRequest>;
            async fn fetch_check_runs(&self, repo: &str, ref_: &str) -> anyhow::Result<Vec<CheckRun>>;
            async fn fetch_timeline(&self, repo: &str, pr_number: u32) -> anyhow::Result<Vec<TimelineEvent>>;
            async fn fetch_rate_limit(&self) -> anyhow::Result<crate::domain::pr::RateLimitStatus>;
            async fn fetch_current_user(&self) -> anyhow::Result<String>;
            async fn open_pr_in_browser(&self, url: &str) -> anyhow::Result<()>;
        }
    }

    mock! {
        pub StateRepository {}
        impl StateRepository for StateRepository {
            fn save_state(&self, prs: &[PullRequest]) -> anyhow::Result<()>;
            fn load_state(&self) -> anyhow::Result<Vec<PullRequest>>;
            fn save_archive(&self, prs: &[PullRequest]) -> anyhow::Result<()>;
            fn load_archive(&self) -> anyhow::Result<Vec<PullRequest>>;
            fn archive_pr(&self, pr: PullRequest) -> anyhow::Result<()>;
        }
    }

    mock! {
        pub Notifier {}
        impl NotificationService for Notifier {
            fn notify_pr_update(&mut self, old_pr: &PullRequest, new_pr: &PullRequest);
            fn notify_new_pr(&mut self, pr: &PullRequest);
            fn clear_cycle(&mut self);
        }
    }

    fn create_test_pr(id: &str, ci: CIStatus) -> PullRequest {
        PullRequest {
            id: id.to_string(),
            number: 1,
            title: "Test".to_string(),
            author: "alice".to_string(),
            repo: "org/repo".to_string(),
            status: PRStatus::Open,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            additions: 0,
            deletions: 0,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: ci,
            mergeable: MergeableStatus::Unknown,
            head_ref: "sha".to_string(),
            body: String::new(),
            url: String::new(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_suppress_notifications_at_startup() {
        let github = Arc::new(MockGithubProvider::new());
        let mut state_repo = MockStateRepository::new();
        state_repo.expect_load_state().returning(|| Ok(vec![]));
        state_repo.expect_load_archive().returning(|| Ok(vec![]));
        state_repo.expect_save_state().returning(|_| Ok(()));

        let state_repo = Arc::new(state_repo);
        let config_dir = std::path::PathBuf::from(".");
        let data_dir = std::path::PathBuf::from(".");
        let backend = ratatui::backend::TestBackend::new(80, 24);

        let mut app = App::with_deps(github, state_repo, &config_dir, &data_dir, backend).unwrap();
        app.is_writer = true;

        let mut notifier = MockNotifier::new();
        // Should NOT be called during first sync
        notifier.expect_notify_new_pr().times(0);
        // Should be called AFTER initial sync
        notifier.expect_notify_new_pr().times(1).returning(|_| ());

        app.notifier = Box::new(notifier);

        let pr = create_test_pr("1", CIStatus::Pending);

        // 1. First sync: Should NOT notify
        app.handle_app_event(AppEvent::PrsUpdated {
            query_name: "test".to_string(),
            prs: vec![pr.clone()],
        })
        .await;

        // 2. Initial sync done
        app.handle_app_event(AppEvent::InitialSyncDone).await;

        // 3. Subsequent update (new PR): SHOULD notify
        let pr2 = create_test_pr("2", CIStatus::Pending);
        app.handle_app_event(AppEvent::PrsUpdated {
            query_name: "test".to_string(),
            prs: vec![pr2],
        })
        .await;
    }
}
