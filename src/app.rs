use crate::domain::pr::{PullRequest, CheckRun, TimelineEvent};
use crate::domain::ports::GithubProvider;
use crate::github::client::GhCliClient;
use crate::ui::events::AppEvent;
use crate::ui::render::Renderer;
use anyhow::Result;
use tokio::sync::mpsc;
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode};

pub struct App {
    pub prs: Vec<PullRequest>,
    pub selected_index: usize,
    pub renderer: Renderer,
    pub event_rx: mpsc::Receiver<AppEvent>,
    pub event_tx: mpsc::Sender<AppEvent>,
    pub should_quit: bool,
    pub github: Box<dyn GithubProvider>,
    pub current_checks: Vec<CheckRun>,
    pub current_timeline: Vec<TimelineEvent>,
}

impl App {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);
        
        Ok(Self {
            prs: Vec::new(),
            selected_index: 0,
            renderer: Renderer::new()?,
            event_rx: rx,
            event_tx: tx,
            should_quit: false,
            github: Box::new(GhCliClient::new()),
            current_checks: Vec::new(),
            current_timeline: Vec::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let tx = self.event_tx.clone();
        
        // Initial fetch (simple for Phase 2)
        let github = GhCliClient::new();
        let initial_prs = github.fetch_prs_by_query("is:open is:pr author:@me").await.unwrap_or_default();
        self.prs = initial_prs;

        // Input task
        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(100)).unwrap()
                    && let Event::Key(key) = event::read().unwrap() {
                        let _ = tx.send(AppEvent::Input(key)).await;
                }
                let _ = tx.send(AppEvent::Tick).await;
            }
        });

        self.renderer.init()?;

        while !self.should_quit {
            // In Phase 2, we just draw with what we have
            self.renderer.draw(&self.prs, self.selected_index)?;

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
                    AppEvent::Error(_) => {
                        // Log error
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
            let github = GhCliClient::new();
            let repo = pr.repo.clone();
            let number = pr.number;
            
            tokio::spawn(async move {
                // Fetch full details to get review status and CI status which might be missing from search
                if let Ok(full_pr) = github.fetch_pr_details(&repo, number).await {
                    let _ = tx.send(AppEvent::PrsUpdated { 
                        query_name: "detail".to_string(), 
                        prs: vec![full_pr] 
                    }).await;
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
