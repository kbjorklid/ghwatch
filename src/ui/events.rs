use crossterm::event::KeyEvent;
use crate::domain::pr::PullRequest;

pub enum AppEvent {
    Tick,
    Input(KeyEvent),
    PrsUpdated { query_name: String, prs: Vec<PullRequest> },
    Error(String),
}
