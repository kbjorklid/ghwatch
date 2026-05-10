use crate::domain::pr::{PullRequest, CheckRun, TimelineEvent};
use crate::config::AppConfig;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    Input(crossterm::event::Event),
    PrsUpdated { query_name: String, prs: Vec<PullRequest> },
    CiStatusLoaded { repo: String, pr_number: u32, checks: Vec<CheckRun> },
    TimelineLoaded { repo: String, pr_number: u32, events: Vec<TimelineEvent> },
    ConfigReloaded(AppConfig),
    StateReloaded(Vec<PullRequest>),
    Error(String),
    PollCycleStarted,
}
