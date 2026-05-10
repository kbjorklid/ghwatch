use crate::domain::pr::{PullRequest, PRStatus};
use chrono::{DateTime, Utc, Duration};

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

    let updated_at = match DateTime::parse_from_rfc3339(&pr.updated_at) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => return false, // If we can't parse, don't unfollow yet
    };

    let now = Utc::now();
    let elapsed = now.signed_duration_since(updated_at);
    
    elapsed >= Duration::minutes(timeout_mins as i64)
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
            url: "".to_string(),
            requested_reviewers: vec![],
            reviewers: vec![],
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

    #[test]
    fn test_should_auto_unfollow() {
        let mut pr = create_test_pr();
        
        // Open PR never unfollows
        pr.status = PRStatus::Open;
        assert!(!should_auto_unfollow(&pr, 0));
        
        // Terminal PR with 0 timeout unfollows immediately
        pr.status = PRStatus::Merged;
        pr.updated_at = Utc::now().to_rfc3339();
        assert!(should_auto_unfollow(&pr, 0));
        
        // Terminal PR with timeout
        let now = Utc::now();
        pr.updated_at = (now - Duration::minutes(10)).to_rfc3339();
        assert!(should_auto_unfollow(&pr, 5));
        assert!(!should_auto_unfollow(&pr, 15));
    }
}
