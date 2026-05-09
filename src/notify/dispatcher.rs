use crate::domain::pr::PullRequest;
use notify_rust::Notification;

pub struct NotificationDispatcher {
    pub enabled: bool,
}

impl NotificationDispatcher {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn notify_pr_update(&self, old_pr: &PullRequest, new_pr: &PullRequest) {
        if !self.enabled {
            return;
        }

        // Check for interesting changes
        if new_pr.review_status != old_pr.review_status {
            self.send_notification(
                &format!("Review Update: #{}", new_pr.number),
                &format!("{} is now {}", new_pr.title, new_pr.review_status),
            );
        }

        if new_pr.ci_status != old_pr.ci_status {
            self.send_notification(
                &format!("CI Update: #{}", new_pr.number),
                &format!("{} CI is now {}", new_pr.title, new_pr.ci_status),
            );
        }

        if new_pr.comment_count > old_pr.comment_count {
            self.send_notification(
                &format!("New Comment: #{}", new_pr.number),
                &format!("{} has {} new comments", new_pr.title, new_pr.comment_count - old_pr.comment_count),
            );
        }
    }

    pub fn notify_new_pr(&self, pr: &PullRequest) {
        if !self.enabled {
            return;
        }

        self.send_notification(
            &format!("New PR: #{}", pr.number),
            &format!("{} by {}", pr.title, pr.author),
        );
    }

    fn send_notification(&self, title: &str, body: &str) {
        let _ = Notification::new()
            .summary(title)
            .body(body)
            .appname("ghnotify")
            .show();
    }
}
