use std::collections::HashSet;
use std::fmt;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::pr::{
    CIStatus, MergeableStatus, PRStatus, PullRequest, ReviewStatus, TimelineEvent,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerReason {
    ReviewRequested,
    ReReviewRequested,
    Mentioned,
    CommentReply,
    CiFailed,
    ChangesRequested,
    MergeConflict,
    Approved,
    NewComments,
}

impl fmt::Display for TriggerReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::ReviewRequested => "Review requested",
            Self::ReReviewRequested => "Re-review requested",
            Self::Mentioned => "Mentioned in comment",
            Self::CommentReply => "Comment reply",
            Self::CiFailed => "CI failed",
            Self::ChangesRequested => "Changes requested",
            Self::MergeConflict => "Merge conflict",
            Self::Approved => "Approved — ready to merge",
            Self::NewComments => "New comments",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DotColor {
    Red,
    Blue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionConfig {
    pub quiet_period_mins: u64,
    pub disabled_reasons: HashSet<TriggerReason>,
    #[serde(default)]
    pub open_in_browser_marks_seen: bool,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            quiet_period_mins: 15,
            disabled_reasons: HashSet::new(),
            open_in_browser_marks_seen: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttentionState {
    #[serde(default)]
    pub active_reasons: HashSet<TriggerReason>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_comment_at: Option<DateTime<Utc>>,
}

impl AttentionState {
    #[must_use]
    pub fn is_red(&self) -> bool {
        !self.active_reasons.is_empty()
    }

    #[must_use]
    pub fn is_blue(&self, pr_updated_at: &str) -> bool {
        if self.is_red() {
            return false;
        }
        match self.last_seen_at {
            None => true,
            Some(seen_at) => parse_ts(pr_updated_at).is_some_and(|ts| ts > seen_at),
        }
    }

    #[must_use]
    pub fn dot_color(&self, pr_updated_at: &str) -> Option<DotColor> {
        if self.is_red() {
            Some(DotColor::Red)
        } else if self.is_blue(pr_updated_at) {
            Some(DotColor::Blue)
        } else {
            None
        }
    }

    pub fn mark_seen(&mut self, now: DateTime<Utc>) {
        self.active_reasons.clear();
        self.last_seen_at = Some(now);
    }

    pub fn remove_reasons(&mut self, reasons: &[TriggerReason]) {
        for r in reasons {
            self.active_reasons.remove(r);
        }
    }
}

pub fn apply_mark_seen(state: &mut AttentionState, now: DateTime<Utc>) {
    state.active_reasons.clear();
    state.last_seen_at = Some(now);
}

pub fn apply_archive(state: &mut AttentionState) {
    state.active_reasons.clear();
}

pub fn apply_user_activity(state: &mut AttentionState) {
    state.active_reasons.clear();
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))
}

fn is_comment_event(e: &TimelineEvent) -> bool {
    e.event_type == "IssueComment" || e.event_type == "PullRequestReview"
}

fn latest_comment_at(timeline: &[TimelineEvent]) -> Option<DateTime<Utc>> {
    timeline.iter().filter(|e| is_comment_event(e)).filter_map(|e| parse_ts(&e.created_at)).max()
}

fn is_newer_than(e: &TimelineEvent, baseline: Option<DateTime<Utc>>) -> bool {
    match baseline {
        None => true,
        Some(t) => parse_ts(&e.created_at).is_some_and(|ts| ts > t),
    }
}

fn update_last_comment_at(state: &mut AttentionState, timeline: &[TimelineEvent]) {
    if let Some(latest) = latest_comment_at(timeline)
        && state.last_comment_at.is_none_or(|old| latest > old)
    {
        state.last_comment_at = Some(latest);
    }
}

#[must_use]
pub fn evaluate(
    prev_state: Option<&AttentionState>,
    prev_pr: Option<&PullRequest>,
    new_pr: &PullRequest,
    timeline: &[TimelineEvent],
    current_user: &str,
    now: DateTime<Utc>,
    config: &AttentionConfig,
) -> AttentionState {
    let is_first = prev_state.is_none();
    let mut state = prev_state.cloned().unwrap_or_default();
    let is_own_pr = new_pr.author == current_user;
    let last_seen = state.last_seen_at;

    // Phase 1: Apply clearing events

    // PR closed/merged → clear all red reasons and return
    if matches!(new_pr.status, PRStatus::Closed | PRStatus::Merged) {
        state.active_reasons.clear();
        update_last_comment_at(&mut state, timeline);
        return state;
    }

    // User posted a new comment → clear all reasons
    if !is_first {
        let user_posted_new_comment = timeline.iter().any(|e| {
            is_comment_event(e) && e.actor == current_user && is_newer_than(e, last_seen)
        });
        if user_posted_new_comment {
            apply_user_activity(&mut state);
        }
    }

    // PR converted to draft → remove ReviewRequested and ReReviewRequested
    if new_pr.is_draft && prev_pr.is_some_and(|p| !p.is_draft) {
        state.remove_reasons(&[TriggerReason::ReviewRequested, TriggerReason::ReReviewRequested]);
    }

    // CI passes → remove CiFailed
    if new_pr.ci_status == CIStatus::Passing {
        state.active_reasons.remove(&TriggerReason::CiFailed);
    }

    // No reviewer has CHANGES_REQUESTED → remove ChangesRequested
    let has_cr_reviewer = new_pr.reviewers.iter().any(|r| r.status == "CHANGES_REQUESTED");
    if !has_cr_reviewer {
        state.active_reasons.remove(&TriggerReason::ChangesRequested);
    }

    // User submitted a new review → remove ReviewRequested, ReReviewRequested
    let user_submitted_new_review = timeline.iter().any(|e| {
        e.event_type == "PullRequestReview"
            && e.actor == current_user
            && is_newer_than(e, last_seen)
    });
    if user_submitted_new_review {
        state.remove_reasons(&[TriggerReason::ReviewRequested, TriggerReason::ReReviewRequested]);
    }

    // Review request removed (user no longer in requested_reviewers) → remove both
    if let Some(pp) = prev_pr {
        let was_requested = pp.requested_reviewers.iter().any(|r| r == current_user);
        let is_requested = new_pr.requested_reviewers.iter().any(|r| r == current_user);
        if was_requested && !is_requested {
            state.remove_reasons(&[
                TriggerReason::ReviewRequested,
                TriggerReason::ReReviewRequested,
            ]);
        }
    }

    // Phase 2: Collect new triggers
    let mut to_add: Vec<TriggerReason> = Vec::new();

    if is_own_pr {
        // CiFailed: transition to Failing (or retroactive on first appearance)
        if new_pr.ci_status == CIStatus::Failing {
            let was_failing = prev_pr.is_some_and(|p| p.ci_status == CIStatus::Failing);
            if is_first || !was_failing {
                to_add.push(TriggerReason::CiFailed);
            }
        }

        // ChangesRequested: transition to ChangesRequested
        if new_pr.review_status == ReviewStatus::ChangesRequested {
            let was_cr = prev_pr.is_some_and(|p| p.review_status == ReviewStatus::ChangesRequested);
            if is_first || !was_cr {
                to_add.push(TriggerReason::ChangesRequested);
            }
        }

        // MergeConflict: transition to Conflicting
        if new_pr.mergeable == MergeableStatus::Conflicting {
            let was_conflicting =
                prev_pr.is_some_and(|p| p.mergeable == MergeableStatus::Conflicting);
            if is_first || !was_conflicting {
                to_add.push(TriggerReason::MergeConflict);
            }
        }

        // Approved: new reviewer moved to APPROVED
        let prev_approved: HashSet<&str> = prev_pr
            .map(|p| {
                p.reviewers
                    .iter()
                    .filter(|r| r.status == "APPROVED")
                    .map(|r| r.login.as_str())
                    .collect()
            })
            .unwrap_or_default();
        let has_new_approved = new_pr
            .reviewers
            .iter()
            .any(|r| r.status == "APPROVED" && !prev_approved.contains(r.login.as_str()));
        if has_new_approved {
            to_add.push(TriggerReason::Approved);
        }

        // NewComments: new comments on own PR with quiet period (retroactive on first appearance)
        let comment_events: Vec<&TimelineEvent> =
            timeline.iter().filter(|e| is_comment_event(e)).collect();
        if !comment_events.is_empty() {
            if is_first {
                to_add.push(TriggerReason::NewComments);
            } else {
                let has_new = comment_events.iter().any(|e| is_newer_than(e, last_seen));
                if has_new {
                    let latest_new_ts = comment_events
                        .iter()
                        .filter(|e| is_newer_than(e, last_seen))
                        .filter_map(|e| parse_ts(&e.created_at))
                        .max();
                    if latest_new_ts.is_some_and(|ts| {
                        now - ts >= Duration::minutes(config.quiet_period_mins as i64)
                    }) {
                        to_add.push(TriggerReason::NewComments);
                    }
                }
            }
        }
    }

    if !is_own_pr {
        // ReviewRequested: user added to requested_reviewers
        let was_requested =
            prev_pr.is_some_and(|p| p.requested_reviewers.iter().any(|r| r == current_user));
        let is_requested = new_pr.requested_reviewers.iter().any(|r| r == current_user);
        if is_requested && (is_first || !was_requested) {
            to_add.push(TriggerReason::ReviewRequested);
        }

        // ReReviewRequested: ReviewRequestedEvent targeting user after user's prior review
        let user_has_prior_review =
            timeline.iter().any(|e| e.event_type == "PullRequestReview" && e.actor == current_user);
        if user_has_prior_review {
            let re_requested = timeline.iter().any(|e| {
                e.event_type == "ReviewRequestedEvent"
                    && e.reviewer_login.as_deref() == Some(current_user)
                    && (is_first || is_newer_than(e, last_seen))
            });
            if re_requested {
                to_add.push(TriggerReason::ReReviewRequested);
            }
        }

        // Mentioned: @user in comment body (not self-mention)
        let mention_pattern = format!("@{current_user}");
        let mentioned = timeline.iter().any(|e| {
            is_comment_event(e)
                && e.actor != current_user
                && e.content.as_ref().is_some_and(|c| c.contains(&mention_pattern))
                && (is_first || is_newer_than(e, last_seen))
        });
        if mentioned {
            to_add.push(TriggerReason::Mentioned);
        }

        // CommentReply: someone else commented on PR where user has commented, discussion quiet
        let user_has_commented =
            timeline.iter().any(|e| is_comment_event(e) && e.actor == current_user);
        if user_has_commented {
            let has_other_new = timeline.iter().any(|e| {
                is_comment_event(e)
                    && e.actor != current_user
                    && (is_first || is_newer_than(e, last_seen))
            });
            if has_other_new {
                if is_first {
                    to_add.push(TriggerReason::CommentReply);
                } else {
                    // Quiet period: measured from latest new comment (any commenter)
                    let latest_new_ts = timeline
                        .iter()
                        .filter(|e| is_comment_event(e) && is_newer_than(e, last_seen))
                        .filter_map(|e| parse_ts(&e.created_at))
                        .max();
                    if latest_new_ts.is_some_and(|ts| {
                        now - ts >= Duration::minutes(config.quiet_period_mins as i64)
                    }) {
                        to_add.push(TriggerReason::CommentReply);
                    }
                }
            }
        }
    }

    // Phase 3: Apply new triggers (respecting disabled_reasons)
    for reason in to_add {
        if config.disabled_reasons.contains(&reason) {
            continue;
        }
        if reason == TriggerReason::ReReviewRequested {
            // ReReviewRequested and ReviewRequested cannot coexist
            state.active_reasons.remove(&TriggerReason::ReviewRequested);
        }
        state.active_reasons.insert(reason);
    }

    // Phase 4: Update last_comment_at to latest comment in timeline
    update_last_comment_at(&mut state, timeline);

    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::Reviewer;

    fn make_pr(author: &str) -> PullRequest {
        PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: author.to_string(),
            repo: "org/repo".to_string(),
            status: PRStatus::Open,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
            additions: 0,
            deletions: 0,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Mergeable,
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

    fn make_event(event_type: &str, actor: &str, created_at: &str) -> TimelineEvent {
        TimelineEvent {
            id: "e1".to_string(),
            event_type: event_type.to_string(),
            actor: actor.to_string(),
            created_at: created_at.to_string(),
            content: None,
            reviewer_login: None,
        }
    }

    fn make_comment(actor: &str, created_at: &str, body: &str) -> TimelineEvent {
        TimelineEvent {
            id: "e1".to_string(),
            event_type: "IssueComment".to_string(),
            actor: actor.to_string(),
            created_at: created_at.to_string(),
            content: Some(body.to_string()),
            reviewer_login: None,
        }
    }

    fn make_review(actor: &str, created_at: &str) -> TimelineEvent {
        TimelineEvent {
            id: "e1".to_string(),
            event_type: "PullRequestReview".to_string(),
            actor: actor.to_string(),
            created_at: created_at.to_string(),
            content: Some("APPROVED:".to_string()),
            reviewer_login: None,
        }
    }

    fn make_reviewer(login: &str, status: &str) -> Reviewer {
        Reviewer { login: login.to_string(), status: status.to_string() }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-02T12:00:00Z").unwrap().with_timezone(&Utc)
    }

    // --- Cycle 1: AttentionState methods ---

    #[test]
    fn test_is_red_empty_set() {
        let state = AttentionState::default();
        assert!(!state.is_red());
    }

    #[test]
    fn test_is_red_with_reason() {
        let mut state = AttentionState::default();
        state.active_reasons.insert(TriggerReason::CiFailed);
        assert!(state.is_red());
    }

    #[test]
    fn test_is_blue_never_seen() {
        let state = AttentionState::default();
        assert!(state.is_blue("2024-01-02T00:00:00Z"));
    }

    #[test]
    fn test_is_blue_updated_after_seen() {
        let mut state = AttentionState::default();
        state.last_seen_at = parse_ts("2024-01-01T10:00:00Z");
        assert!(state.is_blue("2024-01-02T00:00:00Z")); // updated after seen
    }

    #[test]
    fn test_is_blue_seen_after_update() {
        let mut state = AttentionState::default();
        state.last_seen_at = parse_ts("2024-01-03T00:00:00Z");
        assert!(!state.is_blue("2024-01-02T00:00:00Z")); // seen after update
    }

    #[test]
    fn test_is_blue_false_when_red() {
        let mut state = AttentionState::default();
        state.active_reasons.insert(TriggerReason::CiFailed);
        assert!(!state.is_blue("2024-01-02T00:00:00Z")); // red overrides blue
    }

    #[test]
    fn test_dot_color_none() {
        let mut state = AttentionState::default();
        state.last_seen_at = parse_ts("2024-01-03T00:00:00Z"); // seen after update
        assert_eq!(state.dot_color("2024-01-02T00:00:00Z"), None);
    }

    #[test]
    fn test_dot_color_blue() {
        let state = AttentionState::default(); // never seen
        assert_eq!(state.dot_color("2024-01-02T00:00:00Z"), Some(DotColor::Blue));
    }

    #[test]
    fn test_dot_color_red() {
        let mut state = AttentionState::default();
        state.active_reasons.insert(TriggerReason::CiFailed);
        assert_eq!(state.dot_color("2024-01-02T00:00:00Z"), Some(DotColor::Red));
    }

    #[test]
    fn test_mark_seen_clears_and_sets_timestamp() {
        let mut state = AttentionState::default();
        state.active_reasons.insert(TriggerReason::CiFailed);
        state.active_reasons.insert(TriggerReason::Approved);
        let t = now();
        state.mark_seen(t);
        assert!(state.active_reasons.is_empty());
        assert_eq!(state.last_seen_at, Some(t));
    }

    #[test]
    fn test_remove_reasons_targeted() {
        let mut state = AttentionState::default();
        state.active_reasons.insert(TriggerReason::CiFailed);
        state.active_reasons.insert(TriggerReason::Approved);
        state.remove_reasons(&[TriggerReason::CiFailed]);
        assert!(!state.active_reasons.contains(&TriggerReason::CiFailed));
        assert!(state.active_reasons.contains(&TriggerReason::Approved));
    }

    // --- Cycle 2: CiFailed, ChangesRequested, MergeConflict transitions ---

    #[test]
    fn test_ci_failed_fires_on_transition() {
        let mut pr = make_pr("me");
        pr.ci_status = CIStatus::Failing;
        let prev = make_pr("me"); // was Passing
        let s = evaluate(None, Some(&prev), &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::CiFailed));
    }

    #[test]
    fn test_ci_failed_no_refire_while_failing() {
        let mut pr = make_pr("me");
        pr.ci_status = CIStatus::Failing;
        let mut prev = make_pr("me");
        prev.ci_status = CIStatus::Failing;
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::CiFailed]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            Some(&prev),
            &pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        // Still in set (not re-fired, just preserved)
        assert!(s.active_reasons.contains(&TriggerReason::CiFailed));
    }

    #[test]
    fn test_ci_failed_does_not_fire_on_passing() {
        let pr = make_pr("me"); // ci_status = Passing (default)
        let s = evaluate(None, None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::CiFailed));
    }

    #[test]
    fn test_ci_failed_scope_guard_others_pr() {
        let mut pr = make_pr("alice"); // other's PR
        pr.ci_status = CIStatus::Failing;
        let s = evaluate(None, None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::CiFailed));
    }

    #[test]
    fn test_changes_requested_fires_on_transition() {
        let mut pr = make_pr("me");
        pr.review_status = ReviewStatus::ChangesRequested;
        let prev = make_pr("me"); // was Pending
        let s = evaluate(None, Some(&prev), &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::ChangesRequested));
    }

    #[test]
    fn test_changes_requested_no_refire() {
        let mut pr = make_pr("me");
        pr.review_status = ReviewStatus::ChangesRequested;
        pr.reviewers = vec![make_reviewer("bob", "CHANGES_REQUESTED")];
        let mut prev = make_pr("me");
        prev.review_status = ReviewStatus::ChangesRequested;
        prev.reviewers = vec![make_reviewer("bob", "CHANGES_REQUESTED")];
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::ChangesRequested]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            Some(&prev),
            &pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(s.active_reasons.contains(&TriggerReason::ChangesRequested));
    }

    #[test]
    fn test_changes_requested_scope_guard() {
        let mut pr = make_pr("alice");
        pr.review_status = ReviewStatus::ChangesRequested;
        let s = evaluate(None, None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::ChangesRequested));
    }

    #[test]
    fn test_merge_conflict_fires_on_transition() {
        let mut pr = make_pr("me");
        pr.mergeable = MergeableStatus::Conflicting;
        let prev = make_pr("me"); // was Mergeable
        let s = evaluate(None, Some(&prev), &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::MergeConflict));
    }

    #[test]
    fn test_merge_conflict_no_refire() {
        let mut pr = make_pr("me");
        pr.mergeable = MergeableStatus::Conflicting;
        let mut prev = make_pr("me");
        prev.mergeable = MergeableStatus::Conflicting;
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::MergeConflict]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            Some(&prev),
            &pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(s.active_reasons.contains(&TriggerReason::MergeConflict));
    }

    #[test]
    fn test_merge_conflict_scope_guard() {
        let mut pr = make_pr("alice");
        pr.mergeable = MergeableStatus::Conflicting;
        let s = evaluate(None, None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::MergeConflict));
    }

    #[test]
    fn test_first_appearance_retroactive_own_pr() {
        let mut pr = make_pr("me");
        pr.ci_status = CIStatus::Failing;
        pr.review_status = ReviewStatus::ChangesRequested;
        pr.mergeable = MergeableStatus::Conflicting;
        // No prev_state, no prev_pr: first appearance
        let s = evaluate(None, None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::CiFailed));
        assert!(s.active_reasons.contains(&TriggerReason::ChangesRequested));
        assert!(s.active_reasons.contains(&TriggerReason::MergeConflict));
    }

    // --- Cycle 3: Approved ---

    #[test]
    fn test_approved_fires_on_new_approval() {
        let mut pr = make_pr("me");
        pr.reviewers = vec![make_reviewer("bob", "APPROVED")];
        let prev = make_pr("me"); // no reviewers yet
        let s = evaluate(None, Some(&prev), &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::Approved));
    }

    #[test]
    fn test_approved_no_refire_for_existing_approval() {
        let mut pr = make_pr("me");
        pr.reviewers = vec![make_reviewer("bob", "APPROVED")];
        let mut prev = make_pr("me");
        prev.reviewers = vec![make_reviewer("bob", "APPROVED")]; // already approved
        let prev_state = AttentionState::default();
        let s = evaluate(
            Some(&prev_state),
            Some(&prev),
            &pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(!s.active_reasons.contains(&TriggerReason::Approved));
    }

    #[test]
    fn test_approved_scope_guard() {
        let mut pr = make_pr("alice"); // other's PR
        pr.reviewers = vec![make_reviewer("bob", "APPROVED")];
        let s = evaluate(None, None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::Approved));
    }

    #[test]
    fn test_approved_first_appearance_retroactive() {
        let mut pr = make_pr("me");
        pr.reviewers = vec![make_reviewer("bob", "APPROVED")];
        let s = evaluate(None, None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::Approved));
    }

    // --- Cycle 4: ReviewRequested ---

    #[test]
    fn test_review_requested_fires_when_added() {
        let mut pr = make_pr("alice");
        pr.requested_reviewers = vec!["me".to_string()];
        let prev = make_pr("alice"); // not yet requested
        let s = evaluate(None, Some(&prev), &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::ReviewRequested));
    }

    #[test]
    fn test_review_requested_no_refire_if_already_requested() {
        let mut pr = make_pr("alice");
        pr.requested_reviewers = vec!["me".to_string()];
        let mut prev = make_pr("alice");
        prev.requested_reviewers = vec!["me".to_string()]; // already requested
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::ReviewRequested]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            Some(&prev),
            &pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(s.active_reasons.contains(&TriggerReason::ReviewRequested));
    }

    #[test]
    fn test_review_requested_scope_guard_own_pr() {
        let mut pr = make_pr("me"); // own PR
        pr.requested_reviewers = vec!["me".to_string()];
        let s = evaluate(None, None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::ReviewRequested));
    }

    #[test]
    fn test_review_requested_first_appearance() {
        let mut pr = make_pr("alice");
        pr.requested_reviewers = vec!["me".to_string()];
        let s = evaluate(None, None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::ReviewRequested));
    }

    // --- Cycle 5: ReReviewRequested ---

    #[test]
    fn test_re_review_requested_fires_after_prior_review() {
        let pr = make_pr("alice");
        let mut re_request = make_event("ReviewRequestedEvent", "alice", "2024-01-02T01:00:00Z");
        re_request.reviewer_login = Some("me".to_string());
        let prior_review = make_review("me", "2024-01-01T10:00:00Z");
        let timeline = vec![prior_review, re_request];
        let s = evaluate(None, None, &pr, &timeline, "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::ReReviewRequested));
    }

    #[test]
    fn test_re_review_requested_not_for_other_user() {
        let pr = make_pr("alice");
        let mut re_request = make_event("ReviewRequestedEvent", "alice", "2024-01-02T01:00:00Z");
        re_request.reviewer_login = Some("bob".to_string()); // targeting bob, not me
        let prior_review = make_review("me", "2024-01-01T10:00:00Z");
        let timeline = vec![prior_review, re_request];
        let s = evaluate(None, None, &pr, &timeline, "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::ReReviewRequested));
    }

    #[test]
    fn test_re_review_requested_no_fire_without_prior_review() {
        let pr = make_pr("alice");
        let mut re_request = make_event("ReviewRequestedEvent", "alice", "2024-01-02T01:00:00Z");
        re_request.reviewer_login = Some("me".to_string());
        let timeline = vec![re_request]; // no prior review by me
        let s = evaluate(None, None, &pr, &timeline, "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::ReReviewRequested));
    }

    #[test]
    fn test_re_review_requested_scope_guard_own_pr() {
        let pr = make_pr("me"); // own PR
        let mut re_request = make_event("ReviewRequestedEvent", "me", "2024-01-02T01:00:00Z");
        re_request.reviewer_login = Some("me".to_string());
        let prior_review = make_review("me", "2024-01-01T10:00:00Z");
        let timeline = vec![prior_review, re_request];
        let s = evaluate(None, None, &pr, &timeline, "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::ReReviewRequested));
    }

    #[test]
    fn test_re_review_requested_removes_review_requested() {
        let mut pr = make_pr("alice");
        pr.requested_reviewers = vec!["me".to_string()]; // also in requested_reviewers
        let mut re_request = make_event("ReviewRequestedEvent", "alice", "2024-01-02T01:00:00Z");
        re_request.reviewer_login = Some("me".to_string());
        let prior_review = make_review("me", "2024-01-01T10:00:00Z");
        let timeline = vec![prior_review, re_request];
        // Start with ReviewRequested already active
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::ReviewRequested]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &timeline,
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(s.active_reasons.contains(&TriggerReason::ReReviewRequested));
        assert!(!s.active_reasons.contains(&TriggerReason::ReviewRequested));
    }

    // --- Cycle 6: Mentioned ---

    #[test]
    fn test_mentioned_fires_on_at_mention() {
        let pr = make_pr("alice");
        let comment = make_comment("bob", "2024-01-02T01:00:00Z", "Hey @me, please review this");
        let s = evaluate(None, None, &pr, &[comment], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::Mentioned));
    }

    #[test]
    fn test_mentioned_no_fire_for_self_mention() {
        let pr = make_pr("alice");
        let comment = make_comment("me", "2024-01-02T01:00:00Z", "Hey @me, I noted this");
        let s = evaluate(None, None, &pr, &[comment], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::Mentioned));
    }

    #[test]
    fn test_mentioned_scope_guard_own_pr() {
        let pr = make_pr("me"); // own PR
        let comment = make_comment("bob", "2024-01-02T01:00:00Z", "Hey @me, good work");
        let s = evaluate(None, None, &pr, &[comment], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::Mentioned));
    }

    #[test]
    fn test_mentioned_no_fire_for_old_events_after_mark_seen() {
        let pr = make_pr("alice");
        let comment = make_comment("bob", "2024-01-01T10:00:00Z", "Hey @me, please review");
        let prev_state = AttentionState {
            last_seen_at: parse_ts("2024-01-01T12:00:00Z"), // seen after comment
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(!s.active_reasons.contains(&TriggerReason::Mentioned));
    }

    #[test]
    fn test_mentioned_fires_for_bot_comment() {
        let pr = make_pr("alice");
        let comment =
            make_comment("github-actions[bot]", "2024-01-02T01:00:00Z", "Paging @me for review");
        let s = evaluate(None, None, &pr, &[comment], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::Mentioned));
    }

    // --- Cycle 7: CommentReply and NewComments ---

    #[test]
    fn test_comment_reply_fires_after_quiet_period() {
        let pr = make_pr("alice");
        // User commented previously
        let user_comment = make_comment("me", "2024-01-01T10:00:00Z", "LGTM");
        // Someone else commented 20 min ago
        let other_comment = make_comment("bob", "2024-01-02T11:40:00Z", "Please fix this");
        let prev_state = AttentionState {
            last_seen_at: parse_ts("2024-01-01T09:00:00Z"), // seen before user commented
            ..Default::default()
        };
        // now() = 12:00, last comment at 11:40, diff = 20 min > 15 min quiet period
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[user_comment, other_comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(s.active_reasons.contains(&TriggerReason::CommentReply));
    }

    #[test]
    fn test_comment_reply_no_fire_before_quiet_period() {
        let pr = make_pr("alice");
        let user_comment = make_comment("me", "2024-01-01T10:00:00Z", "LGTM");
        // 5 min ago — within quiet period
        let other_comment = make_comment("bob", "2024-01-02T11:55:00Z", "Please fix this");
        let prev_state =
            AttentionState { last_seen_at: parse_ts("2024-01-01T09:00:00Z"), ..Default::default() };
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[user_comment, other_comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(!s.active_reasons.contains(&TriggerReason::CommentReply));
    }

    #[test]
    fn test_comment_reply_scope_guard_own_pr() {
        let pr = make_pr("me"); // own PR
        let user_comment = make_comment("me", "2024-01-01T10:00:00Z", "LGTM");
        let other_comment = make_comment("bob", "2024-01-02T11:40:00Z", "Looks good");
        let prev_state =
            AttentionState { last_seen_at: parse_ts("2024-01-01T09:00:00Z"), ..Default::default() };
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[user_comment, other_comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(!s.active_reasons.contains(&TriggerReason::CommentReply));
    }

    #[test]
    fn test_comment_reply_first_appearance_no_quiet_period() {
        let pr = make_pr("alice");
        // User commented in old PR, someone else also commented — no quiet period on first appearance
        let user_comment = make_comment("me", "2024-01-01T10:00:00Z", "LGTM");
        let other_comment = make_comment("bob", "2024-01-02T11:55:00Z", "Changes needed"); // 5 min ago
        // No prev_state: first appearance
        let s = evaluate(
            None,
            None,
            &pr,
            &[user_comment, other_comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(s.active_reasons.contains(&TriggerReason::CommentReply));
    }

    #[test]
    fn test_new_comments_fires_after_quiet_period() {
        let mut pr = make_pr("me"); // own PR
        pr.comment_count = 1;
        let comment = make_comment("bob", "2024-01-02T11:40:00Z", "Please fix"); // 20 min ago
        let prev_state = AttentionState {
            last_seen_at: parse_ts("2024-01-02T11:00:00Z"), // seen before comment
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(s.active_reasons.contains(&TriggerReason::NewComments));
    }

    #[test]
    fn test_new_comments_no_fire_before_quiet_period() {
        let mut pr = make_pr("me");
        pr.comment_count = 1;
        let comment = make_comment("bob", "2024-01-02T11:55:00Z", "Please fix"); // 5 min ago
        let prev_state =
            AttentionState { last_seen_at: parse_ts("2024-01-02T11:00:00Z"), ..Default::default() };
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(!s.active_reasons.contains(&TriggerReason::NewComments));
    }

    #[test]
    fn test_new_comments_scope_guard_others_pr() {
        let mut pr = make_pr("alice"); // other's PR
        pr.comment_count = 1;
        let comment = make_comment("bob", "2024-01-02T11:40:00Z", "Looks good");
        let prev_state =
            AttentionState { last_seen_at: parse_ts("2024-01-02T11:00:00Z"), ..Default::default() };
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(!s.active_reasons.contains(&TriggerReason::NewComments));
    }

    #[test]
    fn test_new_comments_first_appearance_no_quiet_period() {
        let pr = make_pr("me");
        // 5 min ago comment — would normally be within quiet period
        let comment = make_comment("bob", "2024-01-02T11:55:00Z", "Please fix");
        let s = evaluate(None, None, &pr, &[comment], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::NewComments));
    }

    // --- Cycle 8: Clearing events ---

    #[test]
    fn test_pr_closed_clears_all_red() {
        let mut pr = make_pr("me");
        pr.status = PRStatus::Closed;
        let prev_state = AttentionState {
            active_reasons: HashSet::from([
                TriggerReason::CiFailed,
                TriggerReason::ChangesRequested,
            ]),
            ..Default::default()
        };
        let s =
            evaluate(Some(&prev_state), None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.is_empty());
    }

    #[test]
    fn test_pr_merged_clears_all_red() {
        let mut pr = make_pr("me");
        pr.status = PRStatus::Merged;
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::Approved]),
            ..Default::default()
        };
        let s =
            evaluate(Some(&prev_state), None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.is_empty());
    }

    #[test]
    fn test_ci_passes_clears_only_ci_failed() {
        let pr = make_pr("me"); // ci_status = Passing
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::CiFailed, TriggerReason::Approved]),
            ..Default::default()
        };
        let s =
            evaluate(Some(&prev_state), None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::CiFailed));
        assert!(s.active_reasons.contains(&TriggerReason::Approved));
    }

    #[test]
    fn test_all_reviewers_approved_clears_changes_requested() {
        let mut pr = make_pr("me");
        pr.reviewers = vec![make_reviewer("bob", "APPROVED")]; // no CHANGES_REQUESTED
        let prev_state = AttentionState {
            active_reasons: HashSet::from([
                TriggerReason::ChangesRequested,
                TriggerReason::Approved,
            ]),
            ..Default::default()
        };
        let s =
            evaluate(Some(&prev_state), None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(!s.active_reasons.contains(&TriggerReason::ChangesRequested));
        assert!(s.active_reasons.contains(&TriggerReason::Approved));
    }

    #[test]
    fn test_outstanding_changes_requested_reviewer_keeps_reason() {
        let mut pr = make_pr("me");
        pr.reviewers =
            vec![make_reviewer("bob", "APPROVED"), make_reviewer("carol", "CHANGES_REQUESTED")];
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::ChangesRequested]),
            ..Default::default()
        };
        let s =
            evaluate(Some(&prev_state), None, &pr, &[], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::ChangesRequested));
    }

    #[test]
    fn test_user_submits_review_clears_review_requested() {
        let pr = make_pr("alice");
        let review = make_review("me", "2024-01-02T01:00:00Z"); // new since last_seen_at=None
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::ReviewRequested]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[review],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(!s.active_reasons.contains(&TriggerReason::ReviewRequested));
    }

    #[test]
    fn test_review_request_removed_clears_review_requested() {
        let mut pr = make_pr("alice");
        pr.requested_reviewers = vec![]; // no longer requested
        let mut prev_pr = make_pr("alice");
        prev_pr.requested_reviewers = vec!["me".to_string()]; // was requested
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::ReviewRequested]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            Some(&prev_pr),
            &pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(!s.active_reasons.contains(&TriggerReason::ReviewRequested));
    }

    #[test]
    fn test_apply_mark_seen() {
        let mut state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::CiFailed]),
            ..Default::default()
        };
        let t = now();
        apply_mark_seen(&mut state, t);
        assert!(state.active_reasons.is_empty());
        assert_eq!(state.last_seen_at, Some(t));
    }

    #[test]
    fn test_apply_archive() {
        let mut state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::CiFailed, TriggerReason::Approved]),
            ..Default::default()
        };
        apply_archive(&mut state);
        assert!(state.active_reasons.is_empty());
    }

    // --- Cycle 9: First appearance integration tests ---

    #[test]
    fn test_first_appearance_multiple_reasons_own_pr() {
        let mut pr = make_pr("me");
        pr.ci_status = CIStatus::Failing;
        pr.review_status = ReviewStatus::ChangesRequested;
        let comment = make_comment("bob", "2024-01-01T10:00:00Z", "Please update");
        let s = evaluate(None, None, &pr, &[comment], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::CiFailed));
        assert!(s.active_reasons.contains(&TriggerReason::ChangesRequested));
        assert!(s.active_reasons.contains(&TriggerReason::NewComments));
    }

    #[test]
    fn test_first_appearance_multiple_reasons_others_pr() {
        let mut pr = make_pr("alice");
        pr.requested_reviewers = vec!["me".to_string()];
        let comment = make_comment("bob", "2024-01-01T10:00:00Z", "Hey @me check this");
        let s = evaluate(None, None, &pr, &[comment], "me", now(), &AttentionConfig::default());
        assert!(s.active_reasons.contains(&TriggerReason::ReviewRequested));
        assert!(s.active_reasons.contains(&TriggerReason::Mentioned));
    }

    #[test]
    fn test_no_change_no_new_reasons() {
        let pr = make_pr("me"); // Passing CI, Pending review, Mergeable
        let prev_state = AttentionState::default();
        let s = evaluate(
            Some(&prev_state),
            Some(&pr),
            &pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(s.active_reasons.is_empty());
    }

    #[test]
    fn test_disabled_reason_not_fired() {
        let mut pr = make_pr("me");
        pr.ci_status = CIStatus::Failing;
        let config = AttentionConfig {
            quiet_period_mins: 15,
            disabled_reasons: HashSet::from([TriggerReason::CiFailed]),
            open_in_browser_marks_seen: false,
        };
        let s = evaluate(None, None, &pr, &[], "me", now(), &config);
        assert!(!s.active_reasons.contains(&TriggerReason::CiFailed));
    }

    #[test]
    fn test_last_comment_at_updated() {
        let pr = make_pr("alice");
        let comment = make_comment("bob", "2024-01-02T06:00:00Z", "Nice work");
        let s = evaluate(None, None, &pr, &[comment], "me", now(), &AttentionConfig::default());
        assert_eq!(s.last_comment_at, parse_ts("2024-01-02T06:00:00Z"));
    }

    // --- Cycle 10: Item A — user activity clears all ---

    #[test]
    fn test_user_activity_clears_all_reasons() {
        // own PR with Approved in prev_state (no targeted clear for Approved)
        // user posts a new comment (within quiet period so NewComments won't re-fire)
        // expect all reasons cleared
        let pr = make_pr("me"); // ci_status=Passing, no reviewers
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::Approved]),
            last_seen_at: parse_ts("2024-01-01T09:00:00Z"),
            ..Default::default()
        };
        // 10 min before now() — within the 15-min quiet period, so NewComments won't re-fire
        let user_comment = make_comment("me", "2024-01-02T11:50:00Z", "Working on it");
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[user_comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(s.active_reasons.is_empty(), "User activity should clear all reasons");
    }

    #[test]
    fn test_user_activity_not_fired_on_first_appearance() {
        // first appearance (no prev_state), own PR with CI failing
        // user has an old comment in timeline
        // old comment should NOT clear retroactively-set CiFailed
        let mut pr = make_pr("me");
        pr.ci_status = CIStatus::Failing;
        let user_comment = make_comment("me", "2024-01-01T10:00:00Z", "Working on it");
        let s = evaluate(None, None, &pr, &[user_comment], "me", now(), &AttentionConfig::default());
        assert!(
            s.active_reasons.contains(&TriggerReason::CiFailed),
            "User activity on first appearance must not suppress retroactive triggers"
        );
    }

    // --- Cycle 11: Item B — PR converted to draft clears ReviewRequested / ReReviewRequested ---

    #[test]
    fn test_converted_to_draft_clears_review_requested() {
        // prev_pr non-draft, new_pr draft; prev_state has ReviewRequested + Mentioned
        // expect ReviewRequested gone, Mentioned still present
        let mut prev_pr = make_pr("alice");
        prev_pr.is_draft = false;
        let mut new_pr = make_pr("alice");
        new_pr.is_draft = true;
        let prev_state = AttentionState {
            active_reasons: HashSet::from([
                TriggerReason::ReviewRequested,
                TriggerReason::Mentioned,
            ]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            Some(&prev_pr),
            &new_pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(
            !s.active_reasons.contains(&TriggerReason::ReviewRequested),
            "ReviewRequested should be cleared when PR converts to draft"
        );
        assert!(
            s.active_reasons.contains(&TriggerReason::Mentioned),
            "Mentioned should remain when PR converts to draft"
        );
    }

    #[test]
    fn test_converted_to_draft_no_fire_if_already_draft() {
        // prev_pr draft, new_pr draft (stable state) — no spurious clear
        let mut prev_pr = make_pr("alice");
        prev_pr.is_draft = true;
        let mut new_pr = make_pr("alice");
        new_pr.is_draft = true;
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::ReviewRequested]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            Some(&prev_pr),
            &new_pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(
            s.active_reasons.contains(&TriggerReason::ReviewRequested),
            "ReviewRequested must not be spuriously cleared on stable draft state"
        );
    }

    #[test]
    fn test_non_draft_to_non_draft_no_clear() {
        // prev_pr non-draft, new_pr non-draft — no clear
        let prev_pr = make_pr("alice"); // is_draft defaults to false
        let new_pr = make_pr("alice");
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::ReviewRequested]),
            ..Default::default()
        };
        let s = evaluate(
            Some(&prev_state),
            Some(&prev_pr),
            &new_pr,
            &[],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(
            s.active_reasons.contains(&TriggerReason::ReviewRequested),
            "ReviewRequested must not be cleared in non-draft to non-draft transition"
        );
    }

    #[test]
    fn test_user_activity_not_fired_for_others_comment() {
        // other's PR, Mentioned in prev_state
        // a different user posts a new comment — user activity check should NOT trigger
        let pr = make_pr("alice");
        let prev_state = AttentionState {
            active_reasons: HashSet::from([TriggerReason::Mentioned]),
            last_seen_at: parse_ts("2024-01-01T09:00:00Z"),
            ..Default::default()
        };
        let other_comment = make_comment("bob", "2024-01-02T01:00:00Z", "LGTM");
        let s = evaluate(
            Some(&prev_state),
            None,
            &pr,
            &[other_comment],
            "me",
            now(),
            &AttentionConfig::default(),
        );
        assert!(
            s.active_reasons.contains(&TriggerReason::Mentioned),
            "Others' comments must not clear active reasons"
        );
    }
}
