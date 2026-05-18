use crate::domain::pr::{CheckRun, PullRequest, RateLimitStatus, TimelineEvent};
use anyhow::Result;

#[async_trait::async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait GithubProvider: Send + Sync {
    async fn fetch_prs_by_query(&self, query: &str, limit: Option<u32>)
    -> Result<Vec<PullRequest>>;
    async fn fetch_pr_details(&self, repo: &str, pr_number: u32) -> Result<PullRequest>;
    async fn fetch_check_runs(&self, repo: &str, ref_: &str) -> Result<Vec<CheckRun>>;
    async fn fetch_timeline(&self, repo: &str, pr_number: u32) -> Result<Vec<TimelineEvent>>;
    async fn fetch_rate_limit(&self) -> Result<RateLimitStatus>;
    async fn fetch_current_user(&self) -> Result<String>;
    async fn open_pr_in_browser(&self, url: &str) -> Result<()>;
}

#[cfg_attr(test, mockall::automock)]
pub trait StateRepository: Send + Sync {
    fn load_state(&self) -> Result<Vec<PullRequest>>;
    fn save_state(&self, state: &[PullRequest]) -> Result<()>;
    fn load_archive(&self) -> Result<Vec<PullRequest>>;
    fn save_archive(&self, archive: &[PullRequest]) -> Result<()>;
    fn archive_pr(&self, pr: PullRequest) -> Result<()>;
}

pub trait NotificationService: Send + Sync {
    fn notify_new_pr(&mut self, pr: &PullRequest);
    fn notify_pr_update(&mut self, old_pr: &PullRequest, new_pr: &PullRequest);
    fn clear_cycle(&mut self);
}
