use anyhow::{Result, Context};
use async_trait::async_trait;
use crate::domain::ports::GithubProvider;
use crate::domain::pr::{PullRequest, CheckRun, TimelineEvent};
use crate::github::models::{RawPullRequest, RawCheckRun, RawTimelineEvent};
use tokio::process::Command;
use serde_json;

use crate::github::rate_limit::RateLimitTracker;

pub struct GhCliClient {
    pub rate_limit: RateLimitTracker,
}

impl Default for GhCliClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GhCliClient {
    pub fn new() -> Self {
        Self {
            rate_limit: RateLimitTracker::new(),
        }
    }

    async fn run_gh(&self, args: &[&str]) -> Result<String> {
        let start = std::time::Instant::now();
        let output = Command::new("gh")
            .args(args)
            .output()
            .await
            .context("Failed to execute gh CLI")?;

        let duration = start.elapsed().as_millis() as u64;
        let command_str = format!("gh {}", args.join(" "));
        let exit_code = output.status.code().unwrap_or(-1);
        
        crate::logging::record_gh_call(command_str, exit_code, duration);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("gh CLI error: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl GithubProvider for GhCliClient {
    async fn fetch_prs_by_query(&self, query: &str) -> Result<Vec<PullRequest>> {
        let fields = "id,number,title,author,repository,state,createdAt,updatedAt,body,commentsCount,url";
        let output = self.run_gh(&["search", "prs", query, "--json", fields]).await?;
        
        let raws: Vec<RawPullRequest> = serde_json::from_str(&output)
            .context("Failed to parse gh search prs JSON")?;

        Ok(raws.into_iter().map(Into::into).collect())
    }

    async fn fetch_pr_details(&self, repo: &str, pr_number: u32) -> Result<PullRequest> {
        let fields = "id,number,title,author,headRepository,state,createdAt,updatedAt,body,comments,additions,deletions,reviewDecision,statusCheckRollup,headRefOid,url,reviewRequests,latestReviews";
        let output = self.run_gh(&["pr", "view", &pr_number.to_string(), "-R", repo, "--json", fields]).await?;

        let raw: RawPullRequest = serde_json::from_str(&output)
            .context("Failed to parse gh pr view JSON")?;

        Ok(raw.into())
    }

    async fn fetch_check_runs(&self, repo: &str, ref_: &str) -> Result<Vec<CheckRun>> {
        // gh api repos/{owner}/{repo}/commits/{ref}/check-runs
        let path = format!("repos/{}/commits/{}/check-runs", repo, ref_);
        let output = self.run_gh(&["api", &path]).await?;
        
        let json: serde_json::Value = serde_json::from_str(&output)?;
        let check_runs_raw: Vec<RawCheckRun> = serde_json::from_value(json["check_runs"].clone())?;

        Ok(check_runs_raw.into_iter().map(|r| CheckRun {
            name: r.name,
            status: r.status,
            conclusion: r.conclusion,
            url: r.url,
        }).collect())
    }

    async fn fetch_timeline(&self, repo: &str, pr_number: u32) -> Result<Vec<TimelineEvent>> {
        // gh api repos/{owner}/{repo}/issues/{number}/timeline
        let path = format!("repos/{}/issues/{}/timeline", repo, pr_number);
        let output = self.run_gh(&["api", &path]).await?;

        let raws: Vec<RawTimelineEvent> = serde_json::from_str(&output)?;

        Ok(raws.into_iter().map(|r| {
            let content = match r.typename.as_str() {
                "IssueComment" => r.body.clone(),
                "PullRequestReview" => {
                    let state = r.state.clone().unwrap_or_else(|| "COMMENTED".to_string());
                    let body = r.body.as_ref().map(|b| format!(": {}", b)).unwrap_or_default();
                    Some(format!("{}{}", state, body))
                }
                "PullRequestReviewThread" => Some("started a review thread".to_string()),
                "MergedEvent" => Some("merged this pull request".to_string()),
                "ClosedEvent" => Some("closed this pull request".to_string()),
                "ReopenedEvent" => Some("reopened this pull request".to_string()),
                "LabeledEvent" => r.label.as_ref().map(|l| format!("added label: {}", l.name)),
                "UnlabeledEvent" => r.label.as_ref().map(|l| format!("removed label: {}", l.name)),
                "Commit" => r.message.clone(),
                _ => None,
            };

            TimelineEvent {
                id: r.id.unwrap_or_default(),
                event_type: r.typename,
                actor: r.actor.map(|a| a.login).unwrap_or_else(|| "unknown".to_string()),
                created_at: r.created_at.unwrap_or_default(),
                content,
            }
        }).collect())
    }

    async fn fetch_rate_limit(&self) -> Result<crate::domain::pr::RateLimitStatus> {
        let output = self.run_gh(&["api", "rate_limit"]).await?;
        let json: serde_json::Value = serde_json::from_str(&output)?;
        
        let core = &json["resources"]["core"];
        let status = crate::domain::pr::RateLimitStatus {
            remaining: core["remaining"].as_u64().unwrap_or(0) as u32,
            limit: core["limit"].as_u64().unwrap_or(0) as u32,
            reset_at: core["reset"].as_u64().unwrap_or(0),
        };
        
        self.rate_limit.update(status.clone());
        Ok(status)
    }

    async fn fetch_current_user(&self) -> Result<String> {
        let output = self.run_gh(&["api", "user", "--jq", ".login"]).await?;
        Ok(output.trim().to_string())
    }

    async fn open_pr_in_browser(&self, url: &str) -> Result<()> {
        self.run_gh(&["pr", "view", url, "--web"]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::models::*;
    use crate::domain::pr::{PRStatus, ReviewStatus, CIStatus};

    #[test]
    fn test_raw_to_pr_conversion() {
        let raw = RawPullRequest {
            id: "node_1".to_string(),
            number: 123,
            title: "Test PR".to_string(),
            author: RawAuthor { login: "alice".to_string() },
            repository: Some(RawRepository { name_with_owner: "org/repo".to_string() }),
            head_repository: None,
            state: "OPEN".to_string(),
            created_at: "2024-05-01T10:00:00Z".to_string(),
            updated_at: "2024-05-01T11:00:00Z".to_string(),
            body: "Body text".to_string(),
            comments_count_search: Some(5),
            comments: None,
            additions: Some(10),
            deletions: Some(5),
            review_decision: Some("APPROVED".to_string()),
            status_check_rollup: Some(RawStatusCheckRollup { state: "SUCCESS".to_string() }),
            head_ref_oid: Some("sha123".to_string()),
            url: "https://github.com/org/repo/pull/123".to_string(),
            review_requests: Some(vec![
                RawReviewRequest {
                    requested_reviewer: Some(RawRequestedReviewer {
                        typename: "User".to_string(),
                        login: Some("bob".to_string()),
                    })
                }
            ]),
            latest_reviews: None,
        };

        let pr: PullRequest = raw.into();

        assert_eq!(pr.number, 123);
        assert_eq!(pr.author, "alice");
        assert_eq!(pr.repo, "org/repo");
        assert_eq!(pr.status, PRStatus::Open);
        assert_eq!(pr.review_status, ReviewStatus::Approved);
        assert_eq!(pr.ci_status, CIStatus::Passing);
        assert_eq!(pr.comment_count, 5);
        assert_eq!(pr.additions, 10);
        assert_eq!(pr.deletions, 5);
        assert_eq!(pr.head_ref, "sha123");
        assert_eq!(pr.requested_reviewers, vec!["bob".to_string()]);
    }
}
