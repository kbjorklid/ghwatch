use crate::domain::pr::{CIStatus, PRStatus, PullRequest, ReviewStatus, Reviewer};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct RawPullRequest {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub author: RawAuthor,
    pub repository: Option<RawRepository>,
    #[serde(rename = "headRepository")]
    pub head_repository: Option<RawRepository>,
    pub state: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub body: String,
    #[serde(rename = "commentsCount")]
    pub comments_count: Option<u32>,
    pub review_comments: Option<u32>,
    pub unresolved_count: Option<u32>,
    pub total_resolvable_count: Option<u32>,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    #[serde(rename = "reviewDecision")]
    pub review_decision: Option<String>,
    #[serde(rename = "statusCheckRollup")]
    pub status_check_rollup: Option<RawStatusCheckRollup>,
    #[serde(rename = "headRefOid")]
    pub head_ref_oid: Option<String>,
    pub url: String,
    #[serde(rename = "reviewRequests")]
    pub review_requests: Option<Vec<RawReviewRequest>>,
    #[serde(rename = "latestReviews")]
    pub latest_reviews: Option<Vec<RawReview>>,
}

impl From<RawPullRequest> for PullRequest {
    fn from(raw: RawPullRequest) -> Self {
        let mut reviewers = Vec::new();

        // Add latest reviews
        if let Some(latest) = raw.latest_reviews.as_ref() {
            for review in latest {
                reviewers.push(Reviewer {
                    login: review.author.login.clone(),
                    status: review.state.clone(),
                });
            }
        }

        // Add pending review requests
        if let Some(requests) = raw.review_requests.as_ref() {
            for req in requests {
                if let Some(rr) = req.requested_reviewer.as_ref()
                    && let Some(login) = rr.login.as_ref()
                {
                    // Only add if not already in reviewers (latest reviews take precedence)
                    if !reviewers.iter().any(|r| &r.login == login) {
                        reviewers
                            .push(Reviewer { login: login.clone(), status: "PENDING".to_string() });
                    }
                }
            }
        }

        let requested_reviewers = raw
            .review_requests
            .as_ref()
            .map(|reqs| {
                reqs.iter()
                    .filter_map(|r| r.requested_reviewer.as_ref())
                    .filter_map(|rr| rr.login.clone())
                    .collect()
            })
            .unwrap_or_default();

        let repo = raw
            .repository
            .and_then(|r| if r.name_with_owner.is_empty() { None } else { Some(r.name_with_owner) })
            .or_else(|| {
                raw.head_repository.and_then(|r| {
                    if r.name_with_owner.is_empty() { None } else { Some(r.name_with_owner) }
                })
            })
            .unwrap_or_else(|| {
                // Fallback: parse from URL
                // https://github.com/owner/repo/pull/1
                let parts: Vec<&str> = raw.url.split('/').collect();
                if parts.len() >= 5 {
                    format!("{}/{}", parts[3], parts[4])
                } else {
                    "unknown".to_string()
                }
            });

        Self {
            id: raw.id,
            number: raw.number,
            title: raw.title,
            author: raw.author.login,
            repo,
            status: match raw.state.to_uppercase().as_str() {
                "OPEN" => PRStatus::Open,
                "MERGED" => PRStatus::Merged,
                _ => PRStatus::Closed,
            },
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            additions: raw.additions.unwrap_or(0),
            deletions: raw.deletions.unwrap_or(0),
            review_status: match raw.review_decision.as_deref() {
                Some("APPROVED") => ReviewStatus::Approved,
                Some("CHANGES_REQUESTED") => ReviewStatus::ChangesRequested,
                _ => ReviewStatus::Pending,
            },
            comment_count: raw.comments_count.unwrap_or(0) + raw.review_comments.unwrap_or(0),
            unresolved_count: raw.unresolved_count.unwrap_or(0),
            total_resolvable_count: raw.total_resolvable_count.unwrap_or(0),
            conversational_count: raw.comments_count.unwrap_or(0),
            ci_status: match raw.status_check_rollup.as_ref().map(|s| s.state().to_uppercase()) {
                Some(s) if s == "SUCCESS" => CIStatus::Passing,
                Some(s) if s == "FAILURE" || s == "ERROR" => CIStatus::Failing,
                Some(s) if s == "PENDING" => CIStatus::Pending,
                _ => CIStatus::Skipped,
            },
            head_ref: raw.head_ref_oid.unwrap_or_default(),
            body: raw.body,
            url: raw.url,
            requested_reviewers,
            reviewers,
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawReview {
    pub author: RawAuthor,
    pub state: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawReviewRequest {
    #[serde(rename = "requestedReviewer")]
    pub requested_reviewer: Option<RawRequestedReviewer>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawRequestedReviewer {
    #[serde(rename = "__typename")]
    pub typename: String,
    pub login: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawAuthor {
    pub login: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawRepository {
    #[serde(rename = "nameWithOwner")]
    pub name_with_owner: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawComment {
    pub id: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum RawStatusCheckRollup {
    Summary { state: String },
    List(Vec<serde_json::Value>),
}

impl RawStatusCheckRollup {
    #[must_use]
    pub fn state(&self) -> String {
        match self {
            Self::Summary { state } => state.clone(),
            Self::List(list) => {
                if list.is_empty() {
                    return "EXPECTED".to_string(); // Map to Skipped
                }

                let mut has_failure = false;
                let mut has_pending = false;

                for val in list {
                    if let Some(conclusion) = val.get("conclusion").and_then(|v| v.as_str()) {
                        match conclusion {
                            "SUCCESS" | "NEUTRAL" | "SKIPPED" => {}
                            _ => has_failure = true,
                        }
                    } else if let Some(status) = val.get("status").and_then(|v| v.as_str())
                        && status != "COMPLETED"
                    {
                        has_pending = true;
                    }
                }

                if has_failure {
                    "FAILURE".to_string()
                } else if has_pending {
                    "PENDING".to_string()
                } else {
                    "SUCCESS".to_string()
                }
            }
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawCheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawTimelineEvent {
    pub id: Option<String>,
    #[serde(rename = "__typename")]
    pub typename: String,
    pub actor: Option<RawAuthor>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    pub body: Option<String>,
    pub state: Option<String>,
    pub message: Option<String>,
    pub label: Option<RawLabel>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawLabel {
    pub name: String,
}
