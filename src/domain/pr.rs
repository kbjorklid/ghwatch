use serde::{Deserialize, Serialize};
use derive_more::Display;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub ci_status: CIStatus,
    pub head_ref: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub event_type: String,
    pub actor: String,
    pub created_at: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Display)]
pub enum PRStatus {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Display)]
pub enum ReviewStatus {
    Pending,
    Approved,
    ChangesRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Display)]
pub enum CIStatus {
    Pending,
    Passing,
    Failing,
    Skipped,
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
            ci_status: CIStatus::Passing,
            head_ref: "sha123".to_string(),
            body: "Detailed description".to_string(),
        };

        assert_eq!(pr.number, 42);
        assert_eq!(pr.author, "alice");
    }
}
