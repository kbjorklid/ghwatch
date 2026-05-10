use crate::domain::pr::PullRequest;
use notify_rust::Notification;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum NotificationEvent {
    NewPr,
    CiUpdate,
    ReviewUpdate,
    CommentUpdate,
}

use crate::domain::ports::NotificationService;

pub struct NotificationDispatcher {
    pub enabled: bool,
    last_notifications: HashMap<String, HashSet<NotificationEvent>>,
}

impl NotificationDispatcher {
    pub fn new(enabled: bool) -> Self {
        Self { 
            enabled,
            last_notifications: HashMap::new(),
        }
    }

    fn should_notify(&mut self, pr_id: &str, event: NotificationEvent) -> bool {
        let set = self.last_notifications.entry(pr_id.to_string()).or_default();
        if set.contains(&event) {
            false
        } else {
            set.insert(event);
            true
        }
    }

    fn send_notification(&self, title: &str, body: &str) {
        let _ = Notification::new()
            .summary(title)
            .body(body)
            .appname("ghnotify")
            .show();
    }
}

impl NotificationService for NotificationDispatcher {
    fn clear_cycle(&mut self) {
        self.last_notifications.clear();
    }

    fn notify_pr_update(&mut self, old_pr: &PullRequest, new_pr: &PullRequest) {
        if !self.enabled {
            return;
        }

        // Check for interesting changes
        if new_pr.review_status != old_pr.review_status 
            && self.should_notify(&new_pr.id, NotificationEvent::ReviewUpdate) {
            self.send_notification(
                &format!("Review Update: #{}", new_pr.number),
                &format!("{} is now {}", new_pr.title, new_pr.review_status),
            );
        }

        if new_pr.ci_status != old_pr.ci_status 
            && self.should_notify(&new_pr.id, NotificationEvent::CiUpdate) {
            self.send_notification(
                &format!("CI Update: #{}", new_pr.number),
                &format!("{} CI is now {}", new_pr.title, new_pr.ci_status),
            );
        }

        if new_pr.comment_count > old_pr.comment_count 
            && self.should_notify(&new_pr.id, NotificationEvent::CommentUpdate) {
            self.send_notification(
                &format!("New Comment: #{}", new_pr.number),
                &format!("{} has {} new comments", new_pr.title, new_pr.comment_count - old_pr.comment_count),
            );
        }
    }

    fn notify_new_pr(&mut self, pr: &PullRequest) {
        if !self.enabled {
            return;
        }

        if self.should_notify(&pr.id, NotificationEvent::NewPr) {
            self.send_notification(
                &format!("New PR: #{}", pr.number),
                &format!("{} by {}", pr.title, pr.author),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::{PullRequest, PRStatus, ReviewStatus, CIStatus};

    fn create_test_pr() -> PullRequest {
        PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test".to_string(),
            author: "alice".to_string(),
            repo: "org/repo".to_string(),
            status: PRStatus::Open,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
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
    fn test_notification_logic_disabled() {
        let mut dispatcher = NotificationDispatcher::new(false);
        let pr = create_test_pr();
        dispatcher.notify_new_pr(&pr);
        dispatcher.notify_pr_update(&pr, &pr);
    }

    #[test]
    fn test_notification_deduplication() {
        let mut dispatcher = NotificationDispatcher::new(true);
        let pr = create_test_pr();
        
        // First time should notify (returns true internally in should_notify)
        assert!(dispatcher.should_notify(&pr.id, NotificationEvent::NewPr));
        // Second time same cycle should NOT notify
        assert!(!dispatcher.should_notify(&pr.id, NotificationEvent::NewPr));
        
        // Clear cycle should allow notification again
        dispatcher.clear_cycle();
        assert!(dispatcher.should_notify(&pr.id, NotificationEvent::NewPr));
    }
}
