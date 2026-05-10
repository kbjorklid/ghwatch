use crate::domain::pr::{PullRequest, CheckRun, TimelineEvent};
use crate::domain::ports::{GithubProvider, StateRepository};
use crate::github::client::GhCliClient;
use crate::ui::events::AppEvent;
use crate::ui::render::Renderer;
use crate::config::AppConfig;
use crate::polling::worker::PollingWorker;
use anyhow::Result;
use tokio::sync::mpsc;
use std::time::Duration;
use std::sync::Arc;
use crate::storage::local::FileStateRepository;
use crate::storage::get_data_dir;
use tokio::process::Command;
use crossterm::event;
use ratatui::backend::Backend;

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

pub struct App<B: Backend> 
where 
    B::Error: std::error::Error + Send + Sync + 'static
{
    pub pr_list: PRList,
    pub archive_list: PRList,
    pub settings_selected_index: usize,
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
    pub _lock: Option<FileLock>,
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
}

impl App<ratatui::backend::CrosstermBackend<std::io::Stdout>> {
    pub fn new() -> Result<Self> {
        let config_dir = crate::storage::get_config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let data_dir = get_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        
        let github = Arc::new(GhCliClient::new());
        let state_repo = Arc::new(FileStateRepository::new(data_dir.clone()));
        
        let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
        Self::with_deps(github, state_repo, config_dir, data_dir, backend)
    }
}

impl<B: Backend> App<B> 
where 
    B::Error: std::error::Error + Send + Sync + 'static
{
    pub fn with_deps(
        github: Arc<dyn GithubProvider>,
        state_repo: Arc<dyn StateRepository>,
        config_dir: std::path::PathBuf,
        data_dir: std::path::PathBuf,
        backend: B,
    ) -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);
        let config_path = config_dir.join("config.toml");
        let config = AppConfig::load(&config_path).unwrap_or_default();
        
        let _ = std::fs::create_dir_all(&data_dir);
        let lock_path = data_dir.join(".lock");
        
        let (is_writer, lock) = match FileLock::acquire_exclusive(&lock_path) {
            Ok(lock) => (true, Some(lock)),
            Err(_) => match FileLock::acquire_shared(&lock_path) {
                Ok(lock) => (false, Some(lock)),
                Err(_) => (false, None),
            }
        };

        let prs = state_repo.load_state().unwrap_or_default();
        let archived_prs = state_repo.load_archive().unwrap_or_default();
        let notifier = Box::new(NotificationDispatcher::new(true));

        Ok(Self {
            pr_list: PRList::new(prs),
            archive_list: PRList::new(archived_prs),
            settings_selected_index: 0,
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
        })
    }
}

impl<B: Backend> App<B> 
where 
    B::Error: std::error::Error + Send + Sync + 'static
{
    pub async fn run(&mut self, _init_renderer: bool) -> Result<()> {
        let config_dir = crate::storage::get_config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let config_path = config_dir.join("config.toml");
        if config_path.exists() {
            self.config_watcher = crate::config::watcher::ConfigWatcher::new(&config_path, self.event_tx.clone()).ok();
        }

        if self.config.current_user.is_empty() {
            let output = Command::new("gh")
                .args(["api", "user", "--jq", ".login"])
                .output()
                .await;
            if let Ok(output) = output {
                self.config.current_user = String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }

        if self.is_writer {
            let polling_worker = PollingWorker::new(
                self.config.clone(),
                self.github.clone(),
                self.event_tx.clone(),
            );
            tokio::spawn(polling_worker.start());
        } else {
            let data_dir = get_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let state_path = data_dir.join("state.toml");
            if state_path.exists() {
                self.state_watcher = crate::config::watcher::StateWatcher::new(&state_path, self.event_tx.clone()).ok();
            }
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
                AppMode::Search => (crate::ui::search::filter_prs(self.pr_list.items(), &self.input_buffer), self.pr_list.selected_index()),
                AppMode::Archive => (self.archive_list.items().to_vec(), self.archive_list.selected_index()),
                _ => (self.pr_list.items().to_vec(), self.pr_list.selected_index()),
            };

            self.renderer.draw(crate::ui::render::DrawContext {
                prs: &draw_prs,
                selected_index: draw_index,
                settings_selected_index: self.settings_selected_index,
                checks: &self.current_checks,
                timeline: &self.current_timeline,
                mode: &self.mode,
                detail_focused: self.detail_focused,
                detail_scroll: self.detail_scroll,
                input_buffer: &self.input_buffer,
                config: &self.config,
                last_refresh: self.last_refresh,
            })?;

            if let Some(event) = self.event_rx.recv().await {
                self.handle_app_event(event).await;
            }
        }

        Ok(())
    }

    async fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Input(e) => crate::input::handle_event(self, e).await,
            AppEvent::Tick => {},
            AppEvent::PrsUpdated { query_name, prs } => {
                self.merge_prs(prs, query_name == "detail").await;
                if self.is_writer {
                    let _ = self.state_repo.save_state(self.pr_list.items());
                }
            }
            AppEvent::CiStatusLoaded { repo, pr_number, checks } => {
                if let Some(selected) = self.pr_list.selected_pr()
                    && selected.repo == repo && selected.number == pr_number {
                        self.current_checks = checks;
                    }
            }
            AppEvent::TimelineLoaded { repo, pr_number, events } => {
                if let Some(selected) = self.pr_list.selected_pr()
                    && selected.repo == repo && selected.number == pr_number {
                        self.current_timeline = events;
                    }
            }
            AppEvent::PollCycleStarted => {
                self.notifier.clear_cycle();
            }
            AppEvent::ConfigReloaded(new_config) => {
                self.handle_config_reload(new_config);
            }
            AppEvent::StateReloaded(new_prs) => {
                self.pr_list.set_prs(new_prs);
                self.sort_prs();
            }
            AppEvent::Error(msg) => {
                eprintln!("Error: {}", msg);
            }
        }
    }

    fn handle_config_reload(&mut self, new_config: AppConfig) {
        self.config = new_config;
        if self.is_writer {
            if let Some(task) = self.polling_task.take() {
                task.abort();
            }
            let polling_worker = PollingWorker::new(
                self.config.clone(),
                self.github.clone(),
                self.event_tx.clone(),
            );
            self.polling_task = Some(tokio::spawn(polling_worker.start()));
        }
    }

    pub async fn merge_prs(&mut self, new_prs: Vec<PullRequest>, is_detail: bool) {
        if !is_detail {
            self.last_refresh = Some(std::time::Instant::now());
        }
        
        if is_detail {
            if let Some(new_pr) = new_prs.first()
                && let Some(old_pr) = self.pr_list.items_mut().iter_mut().find(|p| p.id == new_pr.id) {
                    self.notifier.notify_pr_update(old_pr, new_pr);
                    let last_seen = old_pr.last_seen_at.clone();
                    *old_pr = new_pr.clone();
                    old_pr.last_seen_at = last_seen;
            }
        } else {
            let mut current_prs = self.pr_list.items().to_vec();
            for new_pr in new_prs {
                if let Some(old_pr) = current_prs.iter_mut().find(|p| p.id == new_pr.id) {
                    self.notifier.notify_pr_update(old_pr, &new_pr);
                    let last_seen = old_pr.last_seen_at.clone();
                    *old_pr = new_pr.clone();
                    old_pr.last_seen_at = last_seen;
                } else {
                    self.notifier.notify_new_pr(&new_pr);
                    current_prs.push(new_pr);
                }
            }
            self.pr_list.set_prs(current_prs);
            self.sort_prs();
            self.check_auto_unfollow();
            self.trigger_details_fetch().await;
        }
    }

    pub fn check_auto_unfollow(&mut self) {
        if !self.is_writer { return; }
        
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
        let (repo, number) = if input.starts_with("http") {
            let parts: Vec<&str> = input.split('/').collect();
            if parts.len() >= 7 {
                (format!("{}/{}", parts[3], parts[4]), parts[6].parse::<u32>().ok())
            } else {
                return;
            }
        } else if let Some((repo, num_str)) = input.split_once('#') {
            (repo.to_string(), num_str.parse::<u32>().ok())
        } else {
            return;
        };

        if let Some(number) = number {
            let github = self.github.clone();
            let tx = self.event_tx.clone();
            tokio::spawn(async move {
                if let Ok(pr) = github.fetch_pr_details(&repo, number).await {
                    let _ = tx.send(AppEvent::PrsUpdated { 
                        query_name: "manual".to_string(), 
                        prs: vec![pr] 
                    }).await;
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
                    let _ = tx.send(AppEvent::PrsUpdated { 
                        query_name: "detail".to_string(), 
                        prs: vec![full_pr.clone()] 
                    }).await;

                    if !full_pr.head_ref.is_empty() 
                        && let Ok(checks) = github.fetch_check_runs(&repo, &full_pr.head_ref).await {
                            let _ = tx.send(AppEvent::CiStatusLoaded { 
                                repo: repo.clone(), 
                                pr_number: number, 
                                checks 
                            }).await;
                        }
                }

                if let Ok(timeline) = github.fetch_timeline(&repo, number).await {
                    let _ = tx.send(AppEvent::TimelineLoaded { 
                        repo: repo.clone(), 
                        pr_number: number, 
                        events: timeline 
                    }).await;
                }
            });
        }
    }
}
