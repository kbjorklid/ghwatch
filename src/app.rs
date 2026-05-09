use crate::domain::pr::{PullRequest, CheckRun, TimelineEvent};
use crate::domain::ports::GithubProvider;
use crate::github::client::GhCliClient;
use crate::ui::events::AppEvent;
use crate::ui::render::Renderer;
use crate::config::AppConfig;
use crate::polling::worker::PollingWorker;
use anyhow::Result;
use tokio::sync::mpsc;
use std::time::Duration;
use std::sync::Arc;
use crossterm::event::{self, Event, KeyCode};

pub struct App {
    pub prs: Vec<PullRequest>,
    pub selected_index: usize,
    pub renderer: Renderer,
    pub event_rx: mpsc::Receiver<AppEvent>,
    pub event_tx: mpsc::Sender<AppEvent>,
    pub should_quit: bool,
    pub github: Arc<dyn GithubProvider>,
    pub current_checks: Vec<CheckRun>,
    pub current_timeline: Vec<TimelineEvent>,
    pub config: AppConfig,
}

impl App {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);
        let config = AppConfig::default();
        let github = Arc::new(GhCliClient::new());
        
        Ok(Self {
            prs: Vec::new(),
            selected_index: 0,
            renderer: Renderer::new()?,
            event_rx: rx,
            event_tx: tx,
            should_quit: false,
            github,
            current_checks: Vec::new(),
            current_timeline: Vec::new(),
            config,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
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
            self.renderer.draw(&self.prs, self.selected_index, &self.current_checks, &self.current_timeline)?;

            if let Some(event) = self.event_rx.recv().await {
                match event {
                    AppEvent::Input(key) => self.handle_key(key).await,
                    AppEvent::Tick => {},
                    AppEvent::PrsUpdated { query_name, prs } => {
                        if query_name == "detail" {
                            if let Some(new_pr) = prs.first()
                                && let Some(old_pr) = self.prs.get_mut(self.selected_index)
                                && old_pr.id == new_pr.id {
                                    *old_pr = new_pr.clone();
                            }
                        } else {
                            self.prs = prs;
                            self.trigger_details_fetch().await;
                        }
                    }
                    AppEvent::CiStatusLoaded { checks, .. } => {
                        self.current_checks = checks;
                    }
                    AppEvent::TimelineLoaded { events, .. } => {
                        self.current_timeline = events;
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

    async fn handle_key(&mut self, key: event::KeyEvent) {
        let old_index = self.selected_index;
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down if self.selected_index < self.prs.len() - 1 => {
                self.selected_index += 1;
            }
            KeyCode::Char('k') | KeyCode::Up if self.selected_index > 0 => {
                self.selected_index -= 1;
            }
            KeyCode::Char('g') => self.selected_index = 0,
            KeyCode::Char('G') => self.selected_index = self.prs.len().saturating_sub(1),
            _ => {}
        }

        if old_index != self.selected_index {
            self.trigger_details_fetch().await;
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
