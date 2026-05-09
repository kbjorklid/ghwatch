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
use crossterm::event::{self, Event, KeyCode};

use crate::storage::lock::FileLock;

pub enum AppMode {
    Normal,
    Search,
    Follow,
    Settings,
    Archive,
}

use crate::notify::dispatcher::NotificationDispatcher;

pub struct App {
    pub prs: Vec<PullRequest>,
    pub selected_index: usize,
    pub renderer: Renderer,
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
    pub input_buffer: String,
    pub config_watcher: Option<crate::config::watcher::ConfigWatcher>,
    pub notifier: NotificationDispatcher,
}

impl App {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);
        let config = AppConfig::default();
        let github = Arc::new(GhCliClient::new());

        let data_dir = get_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let _ = std::fs::create_dir_all(&data_dir);
        let lock_path = data_dir.join(".lock");
        
        let (is_writer, lock) = match FileLock::acquire_exclusive(&lock_path) {
            Ok(lock) => (true, Some(lock)),
            Err(_) => match FileLock::acquire_shared(&lock_path) {
                Ok(lock) => (false, Some(lock)),
                Err(_) => (false, None),
            }
        };

        let state_repo = Arc::new(FileStateRepository::new(data_dir));
        let prs = state_repo.load_state().unwrap_or_default();
        let notifier = NotificationDispatcher::new(true);

        Ok(Self {
            prs,
            selected_index: 0,
            renderer: Renderer::new()?,
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
            input_buffer: String::new(),
            config_watcher: None,
            notifier,
        })
    }


    pub async fn run(&mut self) -> Result<()> {
        // Initialize config watcher
        let config_dir = crate::storage::get_config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let config_path = config_dir.join("config.toml");
        if config_path.exists() {
            self.config_watcher = crate::config::watcher::ConfigWatcher::new(&config_path, self.event_tx.clone()).ok();
        }

        // Detect current user if missing
        if self.config.current_user.is_empty() {
            let output = Command::new("gh")
                .args(["api", "user", "--jq", ".login"])
                .output()
                .await;
            if let Ok(output) = output {
                self.config.current_user = String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }

        // Start polling worker
        let polling_worker = PollingWorker::new(
            self.config.clone(),
            self.github.clone(),
            self.event_tx.clone(),
        );
        tokio::spawn(polling_worker.start());

        // Input task
        let input_tx = self.event_tx.clone();
        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(100)).unwrap()
                    && let Event::Key(key) = event::read().unwrap() {
                        let _ = input_tx.send(AppEvent::Input(key)).await;
                }
                let _ = input_tx.send(AppEvent::Tick).await;
            }
        });

        self.renderer.init()?;

        while !self.should_quit {
            let filtered_prs = if let AppMode::Search = self.mode {
                crate::ui::search::filter_prs(&self.prs, &self.input_buffer)
            } else {
                self.prs.to_vec()
            };
            self.renderer.draw(&filtered_prs, self.selected_index, &self.current_checks, &self.current_timeline, &self.config.current_user, &self.mode, &self.input_buffer)?;

            if let Some(event) = self.event_rx.recv().await {
                match event {
                    AppEvent::Input(key) => self.handle_key(key).await,
                    AppEvent::Tick => {},
                    AppEvent::PrsUpdated { query_name, prs } => {
                        self.merge_prs(prs, query_name == "detail").await;
                        if self.is_writer {
                            let _ = self.state_repo.save_state(&self.prs);
                        }
                    }
                    AppEvent::CiStatusLoaded { checks, .. } => {
                        self.current_checks = checks;
                    }
                    AppEvent::TimelineLoaded { events, .. } => {
                        self.current_timeline = events;
                    }
                    AppEvent::ConfigReloaded(new_config) => {
                        self.config = new_config;
                    }
                    AppEvent::Error(msg) => {
                        // For now just ignore or log
                        eprintln!("Error: {}", msg);
                    }
                }
            }
        }

        self.renderer.restore()?;
        Ok(())
    }

    async fn merge_prs(&mut self, new_prs: Vec<PullRequest>, is_detail: bool) {
        if is_detail {
            if let Some(new_pr) = new_prs.first()
                && let Some(old_pr) = self.prs.iter_mut().find(|p| p.id == new_pr.id) {
                    self.notifier.notify_pr_update(old_pr, new_pr);
                    let last_seen = old_pr.last_seen_at.clone();
                    *old_pr = new_pr.clone();
                    old_pr.last_seen_at = last_seen;
            }
        } else {
            for new_pr in new_prs {
                if let Some(old_pr) = self.prs.iter_mut().find(|p| p.id == new_pr.id) {
                    self.notifier.notify_pr_update(old_pr, &new_pr);
                    let last_seen = old_pr.last_seen_at.clone();
                    *old_pr = new_pr.clone();
                    old_pr.last_seen_at = last_seen;
                } else {
                    self.notifier.notify_new_pr(&new_pr);
                    self.prs.push(new_pr);
                }
            }
            // Sort by updated_at descending
            self.prs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            self.trigger_details_fetch().await;
        }
    }

    async fn handle_key(&mut self, key: event::KeyEvent) {
        let old_index = self.selected_index;

        match self.mode {
            AppMode::Search => {
                match key.code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.input_buffer.clear();
                    }
                    KeyCode::Enter => {
                        self.mode = AppMode::Normal;
                    }
                    KeyCode::Backspace => {
                        self.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        self.input_buffer.push(c);
                    }
                    _ => {}
                }
                return;
            }
            AppMode::Follow => {
                match key.code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.input_buffer.clear();
                    }
                    KeyCode::Enter => {
                        let input = self.input_buffer.clone();
                        self.input_buffer.clear();
                        self.mode = AppMode::Normal;
                        self.follow_pr(&input).await;
                    }
                    KeyCode::Backspace => {
                        self.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        self.input_buffer.push(c);
                    }
                    _ => {}
                }
                return;
            }
            _ => {}
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('/') => {
                self.mode = AppMode::Search;
            }
            KeyCode::Char('f') if self.is_writer => {
                self.mode = AppMode::Follow;
            }
            KeyCode::Char('S') => {
                self.mode = AppMode::Settings;
            }
            KeyCode::Char('A') => {
                self.mode = AppMode::Archive;
            }
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down if self.selected_index < self.prs.len() - 1 => {
                self.selected_index += 1;
            }
            KeyCode::Char('k') | KeyCode::Up if self.selected_index > 0 => {
                self.selected_index -= 1;
            }
            KeyCode::Char('g') => self.selected_index = 0,
            KeyCode::Char('G') => self.selected_index = self.prs.len().saturating_sub(1),
            KeyCode::Char('m') if self.is_writer => {
                if let Some(pr) = self.prs.get_mut(self.selected_index) {
                    pr.last_seen_at = Some(pr.updated_at.clone());
                    let _ = self.state_repo.save_state(&self.prs);
                }
            }
            KeyCode::Char('M') if self.is_writer => {
                for pr in &mut self.prs {
                    pr.last_seen_at = Some(pr.updated_at.clone());
                }
                let _ = self.state_repo.save_state(&self.prs);
            }
            KeyCode::Char('u') if self.is_writer && self.selected_index < self.prs.len() => {
                let pr = self.prs.remove(self.selected_index);
                if self.selected_index >= self.prs.len() && !self.prs.is_empty() {
                    self.selected_index = self.prs.len() - 1;
                }
                
                // Archive it
                if let Ok(mut archive) = self.state_repo.load_archive() {
                    archive.push(pr);
                    let _ = self.state_repo.save_archive(&archive);
                }
                let _ = self.state_repo.save_state(&self.prs);
            }
            _ => {}
        }

        if old_index != self.selected_index {
            self.trigger_details_fetch().await;
        }
    }

    async fn follow_pr(&mut self, input: &str) {
        // Simple parser
        // https://github.com/owner/repo/pull/123 -> owner, repo, 123
        // owner/repo#123 -> owner, repo, 123
        
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

    async fn trigger_details_fetch(&mut self) {
        if let Some(pr) = self.prs.get(self.selected_index) {
            let tx = self.event_tx.clone();
            let github = self.github.clone();
            let repo = pr.repo.clone();
            let number = pr.number;
            
            tokio::spawn(async move {
                // Fetch full details to get review status and CI status which might be missing from search
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
