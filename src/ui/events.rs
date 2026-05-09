use crossterm::event::KeyEvent;
use crate::domain::pr::{PullRequest, CheckRun, TimelineEvent};

pub enum AppEvent {
    Tick,
    Input(KeyEvent),
    PrsUpdated { query_name: String, prs: Vec<PullRequest> },
    CiStatusLoaded { repo: String, pr_number: u32, checks: Vec<CheckRun> },
    TimelineLoaded { repo: String, pr_number: u32, events: Vec<TimelineEvent> },
    Error(String),
}
