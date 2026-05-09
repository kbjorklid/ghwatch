use crate::domain::pr::{PullRequest, PRStatus, ReviewStatus, CIStatus};
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
}

impl App {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);
        
        let dummy_prs = vec![
            PullRequest {
                id: "1".to_string(),
                number: 101,
                title: "Feat: Add support for custom themes".to_string(),
                author: "kalle".to_string(),
                repo: "google/gemini-cli".to_string(),
                status: PRStatus::Open,
                created_at: "2h ago".to_string(),
                updated_at: "10m ago".to_string(),
                additions: 150,
                deletions: 20,
                review_status: ReviewStatus::Approved,
                comment_count: 5,
                ci_status: CIStatus::Passing,
                body: "# Custom Themes\n\nThis PR adds support for custom themes via a `theme.toml` file.".to_string(),
            },
            PullRequest {
                id: "2".to_string(),
                number: 102,
                title: "Fix: Parser crash on empty input".to_string(),
                author: "bob".to_string(),
                repo: "google/gemini-cli".to_string(),
                status: PRStatus::Open,
                created_at: "5h ago".to_string(),
                updated_at: "1h ago".to_string(),
                additions: 5,
                deletions: 2,
                review_status: ReviewStatus::ChangesRequested,
                comment_count: 12,
                ci_status: CIStatus::Failing,
                body: "Fixes #99. The parser was not handling empty strings correctly.".to_string(),
            },
        ];

        Ok(Self {
            prs: dummy_prs,
            selected_index: 0,
            renderer: Renderer::new()?,
            event_rx: rx,
            event_tx: tx,
            should_quit: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let tx = self.event_tx.clone();
        
        // Input task
        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(100)).unwrap() {
                    if let Event::Key(key) = event::read().unwrap() {
                        let _ = tx.send(AppEvent::Input(key)).await;
                    }
                }
                let _ = tx.send(AppEvent::Tick).await;
            }
        });

        self.renderer.init()?;

        while !self.should_quit {
            self.renderer.draw(&self.prs, self.selected_index)?;

            if let Some(event) = self.event_rx.recv().await {
                match event {
                    AppEvent::Input(key) => self.handle_key(key),
                    AppEvent::Tick => {},
                    _ => {}
                }
            }
        }

        self.renderer.restore()?;
        Ok(())
    }

    fn handle_key(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected_index < self.prs.len() - 1 {
                    self.selected_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Char('g') => self.selected_index = 0,
            KeyCode::Char('G') => self.selected_index = self.prs.len().saturating_sub(1),
            _ => {}
        }
    }
}
