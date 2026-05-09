use anyhow::Result;
use crate::domain::pr::{PullRequest, CheckRun, TimelineEvent, RateLimitStatus};

#[async_trait::async_trait]
pub trait GithubProvider: Send + Sync {
    async fn fetch_prs_by_query(&self, query: &str) -> Result<Vec<PullRequest>>;
    async fn fetch_pr_details(&self, repo: &str, pr_number: u32) -> Result<PullRequest>;
    async fn fetch_check_runs(&self, repo: &str, ref_: &str) -> Result<Vec<CheckRun>>;
    async fn fetch_timeline(&self, repo: &str, pr_number: u32) -> Result<Vec<TimelineEvent>>;
    async fn fetch_rate_limit(&self) -> Result<RateLimitStatus>;
}

pub trait StateRepository: Send + Sync {
    fn load_state(&self) -> Result<Vec<PullRequest>>;
    fn save_state(&self, prs: &[PullRequest]) -> Result<()>;
    fn load_archive(&self) -> Result<Vec<PullRequest>>;
    fn save_archive(&self, prs: &[PullRequest]) -> Result<()>;
}
