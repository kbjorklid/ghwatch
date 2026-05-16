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

#[derive(Debug)]
pub struct NotificationDispatcher {
    pub enabled: bool,
    last_notifications: HashMap<String, HashSet<NotificationEvent>>,
}

impl NotificationDispatcher {
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self { enabled, last_notifications: HashMap::new() }
    }

    pub fn clear_history(&mut self) {
        self.last_notifications.clear();
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

    fn send_notification(title: &str, body: &str) {
        let _ = Notification::new().summary(title).body(body).appname("ghwatch").show();
    }
}

impl NotificationService for NotificationDispatcher {
    fn clear_cycle(&mut self) {
        // We no longer clear the history here to prevent repetitive notifications
        // across poll cycles. The history ensures we only notify once per PR/Event type
        // until something changes or the app restarts.
    }

    fn notify_pr_update(&mut self, old_pr: &PullRequest, new_pr: &PullRequest) {
        if !self.enabled {
            return;
        }

        // Check for interesting changes
        if new_pr.review_status != old_pr.review_status
            && self.should_notify(&new_pr.id, NotificationEvent::ReviewUpdate)
        {
            Self::send_notification(
                &format!("Review Update: #{}", new_pr.number),
                &format!("{} is now {}", new_pr.title, new_pr.review_status),
            );
        }

        if new_pr.ci_status != old_pr.ci_status
            && self.should_notify(&new_pr.id, NotificationEvent::CiUpdate)
        {
            Self::send_notification(
                &format!("CI Update: #{}", new_pr.number),
                &format!("{} CI is now {}", new_pr.title, new_pr.ci_status),
            );
        }

        if new_pr.comment_count > old_pr.comment_count
            && self.should_notify(&new_pr.id, NotificationEvent::CommentUpdate)
        {
            Self::send_notification(
                &format!("New Comment: #{}", new_pr.number),
                &format!(
                    "{} has {} new comments",
                    new_pr.title,
                    new_pr.comment_count - old_pr.comment_count
                ),
            );
        }
    }

    fn notify_new_pr(&mut self, pr: &PullRequest) {
        if !self.enabled {
            return;
        }

        if self.should_notify(&pr.id, NotificationEvent::NewPr) {
            Self::send_notification(
                &format!("New PR: #{}", pr.number),
                &format!("{} by {}", pr.title, pr.author),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, PullRequest, ReviewStatus};

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
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: String::new(),
            body: String::new(),
            url: String::new(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
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

        // First time should notify
        assert!(dispatcher.should_notify(&pr.id, NotificationEvent::NewPr));
        // Second time should NOT notify (persists across cycles)
        assert!(!dispatcher.should_notify(&pr.id, NotificationEvent::NewPr));

        // clear_cycle should NOT clear history now
        dispatcher.clear_cycle();
        assert!(!dispatcher.should_notify(&pr.id, NotificationEvent::NewPr));

        // clear_history should clear it
        dispatcher.clear_history();
        assert!(dispatcher.should_notify(&pr.id, NotificationEvent::NewPr));
    }
}
