use serde::Deserialize;
use crate::domain::pr::{PullRequest, PRStatus, ReviewStatus, CIStatus, Reviewer};

#[derive(Debug, Deserialize)]
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
    pub comments_count_search: Option<u32>,
    pub comments: Option<Vec<RawComment>>,
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
                    && let Some(login) = rr.login.as_ref() {
                    // Only add if not already in reviewers (latest reviews take precedence)
                    if !reviewers.iter().any(|r| &r.login == login) {
                        reviewers.push(Reviewer {
                            login: login.clone(),
                            status: "PENDING".to_string(),
                        });
                    }
                }
            }
        }

        let requested_reviewers = raw.review_requests.as_ref().map(|reqs| {
            reqs.iter()
                .filter_map(|r| r.requested_reviewer.as_ref())
                .filter_map(|rr| rr.login.clone())
                .collect()
        }).unwrap_or_default();

        PullRequest {
            id: raw.id,
            number: raw.number,
            title: raw.title,
            author: raw.author.login,
            repo: raw.repository.map(|r| r.name_with_owner)
                .or(raw.head_repository.map(|r| r.name_with_owner))
                .unwrap_or_default(),
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
            comment_count: raw.comments_count_search.or(raw.comments.map(|c| c.len() as u32)).unwrap_or(0),
            ci_status: match raw.status_check_rollup.as_ref().map(|s| s.state.to_uppercase()) {
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
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RawReview {
    pub author: RawAuthor,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct RawReviewRequest {
    #[serde(rename = "requestedReviewer")]
    pub requested_reviewer: Option<RawRequestedReviewer>,
}

#[derive(Debug, Deserialize)]
pub struct RawRequestedReviewer {
    #[serde(rename = "__typename")]
    pub typename: String,
    pub login: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawAuthor {
    pub login: String,
}

#[derive(Debug, Deserialize)]
pub struct RawRepository {
    #[serde(rename = "nameWithOwner")]
    pub name_with_owner: String,
}

#[derive(Debug, Deserialize)]
pub struct RawComment {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct RawStatusCheckRollup {
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct RawCheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub url: String,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct RawLabel {
    pub name: String,
}
