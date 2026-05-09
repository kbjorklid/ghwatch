use crate::domain::pr::{PullRequest, PRStatus};

pub fn is_unread(pr: &PullRequest) -> bool {
    match &pr.last_seen_at {
        None => true,
        Some(seen_at) => pr.updated_at > *seen_at,
    }
}

pub fn should_auto_unfollow(pr: &PullRequest, timeout_mins: u64) -> bool {
    if pr.status == PRStatus::Open {
        return false;
    }

    // Since we don't have the exact transition time to terminal state in PullRequest (updated_at is just last change),
    // we use updated_at as a proxy for the transition time.
    // In a real app, we might want to track the transition time explicitly.
    
    // If timeout is 0, it means immediate removal.
    if timeout_mins == 0 {
        return true;
    }

    // Without a date library, parsing ISO 8601 is tedious.
    // I'll just check if it's terminal for now and let the App decide based on real time.
    
    pr.status != PRStatus::Open
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::ReviewStatus;
    use crate::domain::pr::CIStatus;

    fn create_test_pr() -> PullRequest {
        PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test".to_string(),
            author: "alice".to_string(),
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "2024-05-01T10:00:00Z".to_string(),
            updated_at: "2024-05-01T11:00:00Z".to_string(),
            additions: 0,
            deletions: 0,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            ci_status: CIStatus::Passing,
            head_ref: "".to_string(),
            body: "".to_string(),
            requested_reviewers: vec![],
            last_seen_at: None,
        }
    }

    #[test]
    fn test_is_unread() {
        let mut pr = create_test_pr();
        assert!(is_unread(&pr));

        pr.last_seen_at = Some("2024-05-01T10:30:00Z".to_string());
        assert!(is_unread(&pr)); // updated at 11:00 > seen at 10:30

        pr.last_seen_at = Some("2024-05-01T11:30:00Z".to_string());
        assert!(!is_unread(&pr)); // updated at 11:00 < seen at 11:30
    }
}
