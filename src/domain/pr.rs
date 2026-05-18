use derive_more::Display;
use serde::{Deserialize, Serialize};

use crate::domain::attention::AttentionState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub author: String,
    pub repo: String,
    pub status: PRStatus,
    pub created_at: String,
    pub updated_at: String,
    pub additions: u32,
    pub deletions: u32,
    pub review_status: ReviewStatus,
    pub comment_count: u32,
    pub unresolved_count: u32,
    pub total_resolvable_count: u32,
    pub conversational_count: u32,
    pub ci_status: CIStatus,
    pub mergeable: MergeableStatus,
    pub head_ref: String,
    pub body: String,
    pub url: String,
    pub requested_reviewers: Vec<String>,
    pub reviewers: Vec<Reviewer>,
    #[serde(default)]
    pub is_draft: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_queries: Vec<String>,
    pub last_seen_at: Option<String>,
    pub last_seen_unresolved_count: u32,
    pub last_seen_total_resolvable_count: u32,
    pub last_seen_conversational_count: u32,
    #[serde(default)]
    pub attention_state: AttentionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reviewer {
    pub login: String,
    pub status: String, // "APPROVED", "CHANGES_REQUESTED", "COMMENTED", "PENDING"
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub event_type: String,
    pub actor: String,
    pub created_at: String,
    pub content: Option<String>,
    #[serde(default)]
    pub reviewer_login: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
pub enum PRStatus {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
pub enum ReviewStatus {
    Pending,
    Approved,
    ChangesRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
pub enum CIStatus {
    Pending,
    Passing,
    Failing,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
pub enum MergeableStatus {
    Mergeable,
    Conflicting,
    BlockedByRequirements,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    pub remaining: u32,
    pub limit: u32,
    pub reset_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pr_creation() {
        let pr = PullRequest {
            id: "node_123".to_string(),
            number: 42,
            title: "Fix bug in parser".to_string(),
            author: "alice".to_string(),
            repo: "org/repo".to_string(),
            status: PRStatus::Open,
            created_at: "2024-05-01T10:00:00Z".to_string(),
            updated_at: "2024-05-01T11:00:00Z".to_string(),
            additions: 100,
            deletions: 50,
            review_status: ReviewStatus::Pending,
            comment_count: 3,
            unresolved_count: 1,
            total_resolvable_count: 2,
            conversational_count: 1,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Mergeable,
            head_ref: "sha123".to_string(),
            body: "Detailed description".to_string(),
            url: "https://github.com/org/repo/pull/42".to_string(),
            requested_reviewers: vec!["bob".to_string()],
            reviewers: vec![],
            is_draft: false,
            matched_queries: Vec::new(),
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: AttentionState::default(),
        };

        assert_eq!(pr.number, 42);
        assert_eq!(pr.author, "alice");
    }
}
