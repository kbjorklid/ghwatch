use crate::domain::attention::AttentionState;
use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, PullRequest, ReviewStatus, Reviewer};
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
    pub mergeable: Option<String>,
    #[serde(rename = "statusCheckRollup")]
    pub status_check_rollup: Option<RawStatusCheckRollup>,
    #[serde(rename = "headRefOid")]
    pub head_ref_oid: Option<String>,
    pub url: String,
    #[serde(rename = "reviewRequests")]
    pub review_requests: Option<Vec<RawReviewRequest>>,
    #[serde(rename = "latestReviews")]
    pub latest_reviews: Option<Vec<RawReview>>,
    #[serde(rename = "isDraft", default)]
    pub is_draft: bool,
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
            mergeable: match raw.mergeable.as_deref() {
                Some("MERGEABLE") => MergeableStatus::Mergeable,
                Some("CONFLICTING") => MergeableStatus::Conflicting,
                _ => MergeableStatus::Unknown,
            },
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
            is_draft: raw.is_draft,
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: AttentionState::default(),
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
    pub node_id: Option<String>,
    #[serde(rename = "event")]
    pub typename: String,
    #[serde(alias = "user")]
    pub actor: Option<RawAuthor>,
    #[serde(rename = "created_at", alias = "submitted_at")]
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

#[cfg(test)]
mod timeline_deserialization_tests {
    use super::*;

    fn parse(json: &str) -> Vec<RawTimelineEvent> {
        serde_json::from_str(json).expect("timeline JSON should parse")
    }

    #[test]
    fn test_timeline_commented_event() {
        let json = r#"[{
            "id": 4415964589,
            "node_id": "IC_kwDOSZQ-mc8AAAABBzZFrQ",
            "event": "commented",
            "actor": {"login": "bob"},
            "body": "Looks good!",
            "created_at": "2024-01-01T11:00:00Z"
        }]"#;
        let events = parse(json);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.typename, "commented");
        assert_eq!(e.node_id.as_deref(), Some("IC_kwDOSZQ-mc8AAAABBzZFrQ"));
        assert_eq!(e.actor.as_ref().map(|a| a.login.as_str()), Some("bob"));
        assert_eq!(e.body.as_deref(), Some("Looks good!"));
        assert_eq!(e.created_at.as_deref(), Some("2024-01-01T11:00:00Z"));
    }

    #[test]
    fn test_timeline_reviewed_event_uses_user_and_submitted_at() {
        // Reviews use `user` (not `actor`) and `submitted_at` (not `created_at`).
        let json = r#"[{
            "id": 4259315044,
            "node_id": "PRR_kwDOSZQ-mc793_1k",
            "event": "reviewed",
            "user": {"login": "alice", "id": 123, "node_id": "MDQ6VXNlcjEyMw=="},
            "state": "approved",
            "body": "LGTM",
            "submitted_at": "2024-01-01T10:00:00Z"
        }]"#;
        let events = parse(json);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.typename, "reviewed");
        assert_eq!(e.actor.as_ref().map(|a| a.login.as_str()), Some("alice"));
        assert_eq!(e.state.as_deref(), Some("approved"));
        assert_eq!(e.created_at.as_deref(), Some("2024-01-01T10:00:00Z"));
    }

    #[test]
    fn test_timeline_committed_event_no_actor_no_timestamp() {
        // Commits have no `actor`/`user` and no `created_at`/`submitted_at`.
        let json = r#"[{
            "sha": "e74bc4ffd8c5243113f5fa18d7e065e6327b40fd",
            "node_id": "C_kwDOSZQ-mdoAKGU3NGJj",
            "event": "committed",
            "message": "Add multi-line test file",
            "author": {"name": "Alice", "email": "alice@example.com", "date": "2024-01-01T09:00:00Z"},
            "committer": {"name": "Alice", "email": "alice@example.com", "date": "2024-01-01T09:00:00Z"}
        }]"#;
        let events = parse(json);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.typename, "committed");
        assert!(e.actor.is_none());
        assert!(e.created_at.is_none());
        assert_eq!(e.message.as_deref(), Some("Add multi-line test file"));
    }

    #[test]
    fn test_timeline_merged_event() {
        let json = r#"[{
            "id": 999,
            "node_id": "ME_abc",
            "event": "merged",
            "actor": {"login": "alice"},
            "created_at": "2024-02-01T12:00:00Z"
        }]"#;
        let events = parse(json);
        assert_eq!(events[0].typename, "merged");
        assert_eq!(events[0].actor.as_ref().map(|a| a.login.as_str()), Some("alice"));
    }

    #[test]
    fn test_timeline_closed_and_reopened_events() {
        let json = r#"[
            {"node_id":"CE_1","event":"closed","actor":{"login":"alice"},"created_at":"2024-03-01T00:00:00Z"},
            {"node_id":"RE_1","event":"reopened","actor":{"login":"alice"},"created_at":"2024-03-02T00:00:00Z"}
        ]"#;
        let events = parse(json);
        assert_eq!(events[0].typename, "closed");
        assert_eq!(events[1].typename, "reopened");
    }

    #[test]
    fn test_timeline_labeled_and_unlabeled_events() {
        let json = r#"[
            {"node_id":"LE_1","event":"labeled","actor":{"login":"alice"},"label":{"name":"bug","color":"fc2929"},"created_at":"2024-03-01T00:00:00Z"},
            {"node_id":"UE_1","event":"unlabeled","actor":{"login":"alice"},"label":{"name":"wip","color":"aabbcc"},"created_at":"2024-03-02T00:00:00Z"}
        ]"#;
        let events = parse(json);
        assert_eq!(events[0].label.as_ref().map(|l| l.name.as_str()), Some("bug"));
        assert_eq!(events[1].label.as_ref().map(|l| l.name.as_str()), Some("wip"));
    }

    #[test]
    fn test_timeline_full_array_from_live_response() {
        // Real shape observed from kbjorklid/gh-notify-test PR #2.
        let json = r#"[
            {
                "sha":"e74bc4ffd8c5243113f5fa18d7e065e6327b40fd",
                "node_id":"C_kwDOSZQ-mdoAKGU3NGJjNGZmZDhjNTI0MzExM2Y1ZmExOGQ3ZTA2NWU2MzI3YjQwZmQ",
                "event":"committed","message":"Add multi-line test file",
                "author":{"name":"Kalle","email":"k@example.com","date":"2024-01-01T09:00:00Z"}
            },
            {
                "id":4259315044,"node_id":"PRR_kwDOSZQ-mc793_1k",
                "event":"reviewed",
                "user":{"login":"kbjorklid","id":4545359,"node_id":"MDQ6VXNlcjQ1NDUzNTk="},
                "state":"commented","body":null,
                "submitted_at":"2026-05-10T11:45:27Z"
            },
            {
                "id":4415964589,"node_id":"IC_kwDOSZQ-mc8AAAABBzZFrQ",
                "event":"commented",
                "actor":{"login":"kbjorklid","id":4545359,"node_id":"MDQ6VXNlcjQ1NDUzNTk="},
                "body":"This is a test conversational comment",
                "created_at":"2026-05-10T18:02:09Z"
            },
            {
                "id":4303562009,"node_id":"PRR_kwDOSZQ-mc8AAAABAIMlGQ",
                "event":"reviewed",
                "user":{"login":"kbjorklid","id":4545359,"node_id":"MDQ6VXNlcjQ1NDUzNTk="},
                "state":"commented","body":null,
                "submitted_at":"2026-05-16T11:00:36Z"
            }
        ]"#;
        let events = parse(json);
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].typename, "committed");
        assert_eq!(events[1].typename, "reviewed");
        assert_eq!(events[1].node_id.as_deref(), Some("PRR_kwDOSZQ-mc793_1k"));
        assert_eq!(events[1].actor.as_ref().map(|a| a.login.as_str()), Some("kbjorklid"));
        assert_eq!(events[1].created_at.as_deref(), Some("2026-05-10T11:45:27Z"));
        assert_eq!(events[2].typename, "commented");
        assert_eq!(events[2].body.as_deref(), Some("This is a test conversational comment"));
        assert_eq!(events[3].created_at.as_deref(), Some("2026-05-16T11:00:36Z"));
    }
}
