use crate::domain::pr::{PullRequest, ReviewStatus, CIStatus};

pub fn needs_attention(pr: &PullRequest, current_user: &str) -> bool {
    // 1. Changes Requested — someone requested changes on your PR.
    let changes_requested = pr.author == current_user && pr.review_status == ReviewStatus::ChangesRequested;
    
    // 2. CI Failed — GitHub Actions checks failed on your PR.
    let ci_failed = pr.author == current_user && pr.ci_status == CIStatus::Failing;
    
    // 3. Pending Review — you are requested as a reviewer and haven't responded.
    let pending_review = pr.requested_reviewers.iter().any(|r| r == current_user);

    changes_requested || ci_failed || pending_review
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::PRStatus;

    fn create_test_pr() -> PullRequest {
        PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test".to_string(),
            author: "alice".to_string(),
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "".to_string(),
            updated_at: "".to_string(),
            additions: 0,
            deletions: 0,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            head_ref: "".to_string(),
            body: "".to_string(),
            url: "".to_string(),
            requested_reviewers: vec![],
            reviewers: vec![],
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
        }
    }

    #[test]
    fn test_changes_requested() {
        let mut pr = create_test_pr();
        pr.author = "me".to_string();
        pr.review_status = ReviewStatus::ChangesRequested;
        assert!(needs_attention(&pr, "me"));
        assert!(!needs_attention(&pr, "other"));
    }

    #[test]
    fn test_ci_failed() {
        let mut pr = create_test_pr();
        pr.author = "me".to_string();
        pr.ci_status = CIStatus::Failing;
        assert!(needs_attention(&pr, "me"));
        assert!(!needs_attention(&pr, "other"));
    }

    #[test]
    fn test_pending_review() {
        let mut pr = create_test_pr();
        pr.requested_reviewers = vec!["me".to_string()];
        assert!(needs_attention(&pr, "me"));
        assert!(!needs_attention(&pr, "alice"));
    }
}
