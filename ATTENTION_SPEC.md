# PR Attention Model — Design Spec

## Problem

The current `!` marker fires on change requests and never clears until the reviewer approves. It cannot distinguish "I haven't seen this" from "I know, I'm working on it." Marking a PR as seen has no effect.

## Design

Replace the two markers (`●` unread, `!` attention) with a single dot. Color encodes severity:

| Dot  | Meaning                                        |
|------|------------------------------------------------|
| None | Nothing new since you last looked              |
| Blue | New activity that does not require action      |
| Red  | Something new that requires your action        |

Marking a PR as seen always clears the dot.

The comment delta display (`old+new` unresolved/conversational counts) is preserved independently of the dot.

## Data Model

Each PR carries a **set of active trigger reasons**. The red dot is shown when any red-trigger reason is in the set. The blue dot is a derived state — it shows when `updated_at > last_seen_at` and the set is empty. Marking a PR as seen clears the entire set and resets `last_seen_at`.

Trigger reasons fire once and stay in the set until a clearing event removes them. Some clearing events are targeted — they remove only the specific reason they correspond to, leaving other active reasons intact. Others are blanket clears.

| Reason | Color | Applies to |
|---|---|---|
| `ReviewRequested` | Red | Other's PRs |
| `ReReviewRequested` | Red | Other's PRs |
| `Mentioned` | Red | Other's PRs |
| `CommentReply` | Red | Other's PRs |
| `CiFailed` | Red | Your PRs |
| `ChangesRequested` | Red | Your PRs |
| `MergeConflict` | Red | Your PRs |
| `Approved` | Red | Your PRs |
| `NewComments` | Red | Your PRs |

When multiple red reasons are active, the dot stays red until all are cleared. The active reasons are shown in the detail view so you can see why the dot is red.

## Attention Triggers

The dot turns red when **something new** requires your action — not when a condition merely persists. Each trigger fires once. Marking as seen suppresses it until the next qualifying event.

### Retroactive firing on first appearance

When a PR first appears in the list (on first launch or first poll), all state-based triggers evaluate the PR's current state and fire immediately if the condition is met. `NewComments` and `CommentReply` also fire retroactively on first appearance if their conditions are met, without applying the quiet period (the comments are old; no settling wait is needed).

### Other people's PRs

**You are requested as reviewer.** (`ReviewRequested`) Fires on the polling cycle where the review request is first detected, including retroactively on first appearance.

**You are re-requested as reviewer.** (`ReReviewRequested`) The author explicitly re-requested your review (via GitHub's "Re-request review" action) after you previously submitted a review. A new push alone does not trigger this — the explicit re-request action is required. When `ReReviewRequested` fires, it removes `ReviewRequested` from the set first; the two reasons cannot be active simultaneously. `ReReviewRequested` is treated as a new trigger even if you reviewed before.

**You are mentioned by name in a comment.** (`Mentioned`) Fires when your handle appears in a comment body — either an inline review comment or a top-level PR comment. Does not fire for mentions in the PR title or description. Does not fire if you are the author of the comment (self-mentions suppressed). Bot comments that mention you do count.

**Someone commented on a PR where you have commented, and the discussion has been quiet for the configured quiet period.** (`CommentReply`) Covers both inline review threads and top-level PR comments. The quiet period is per-PR: any new comment on the PR — including your own and bot comments — resets the clock, regardless of which thread it lands in. There is no maximum cap; if a discussion is continuously active, this trigger is indefinitely suppressed until the conversation goes quiet. The delay lets commenting settle so you see the full picture before acting.

### Your PRs

**CI failed on your PR.** (`CiFailed`) Fires on transition from passing/pending to failing. Does not re-fire on every poll while CI remains failing. Re-fires if CI fails again after a new push — each push resets eligibility for a future `CiFailed` trigger. `Approved` is red because it requires action: you should merge (or assess why you can't).

**Changes were requested on your PR.** (`ChangesRequested`) Fires on transition from no-changes-requested to changes-requested. Does not re-fire on every poll while the condition persists. Once you mark as seen, the trigger is suppressed until the next qualifying event (e.g., a reviewer who had approved now requests changes).

**Your PR has a merge conflict.** (`MergeConflict`) Fires on transition from mergeable to conflicting. Does not re-fire on every poll while the conflict persists. Once you mark as seen, suppressed until the conflict is resolved and then reappears.

**Your PR was approved.** (`Approved`) Fires when a reviewer submits an approving review. Red because it requires action: you should merge (or assess why you cannot). Fires on each new approving review event.

**New comments arrived since you last marked as seen, and the discussion has been quiet for the configured quiet period.** (`NewComments`) Bot comments count. The quiet period and clock-reset rules are identical to `CommentReply`. Fires retroactively on first PR appearance (without quiet period) if comments exist that you have never seen.

## Events that clear the dot

| Clearing event | Reasons removed | Notes |
|---|---|---|
| Mark-as-seen (manual) | All | Resets `last_seen_at`; clears entire set |
| Archiving the PR | All | Same effect as mark-as-seen |
| Your activity on the PR | All (red + blue) | "Activity" means you posted a comment (inline or top-level). Pushing a commit does not count. |
| PR closed or merged | All red reasons | Blue dot persists if `updated_at > last_seen_at` — you may still want to see the final state |
| PR converted to draft | `ReviewRequested`, `ReReviewRequested` | Other active reasons (e.g., `Mentioned`) remain |
| CI passes (same commit) | `CiFailed` | Targeted: other reasons unaffected |
| All change-requesting reviewers have approved or been dismissed | `ChangesRequested` | A single approval does not clear this if another reviewer's change-request is still outstanding |
| You submit your review | `ReviewRequested`, `ReReviewRequested` | Any review type counts: approve, request changes, or comment |
| Your review request is removed | `ReviewRequested`, `ReReviewRequested` | Both cleared regardless of which reason was active |

## Implementation Notes

These notes record findings from codebase investigation to inform Phase 1 design decisions.

### Transition detection (CiFailed, ChangesRequested, MergeConflict, Approved)

`merge_prs()` in `app.rs` already has both `old_pr` and `new_pr` in scope before the assignment overwrites `old_pr`. Transition checks (e.g. `old_pr.ci_status != Failing && new_pr.ci_status == Failing`) can be evaluated directly. `evaluate()` should accept `prev_pr: Option<&PullRequest>` — `None` signals first appearance.

For `CiFailed` re-fire after a new push: a new push always transitions CI through `Pending` before potentially reaching `Failing` again. The `Pending → Failing` transition naturally re-fires the trigger without explicit SHA tracking. No `head_sha` field is needed.

### Mentioned trigger

`TimelineEvent.content` already contains comment body text for `IssueComment` and `PullRequestReview` events (populated by the existing `fetch_timeline()` implementation in `src/github/client.rs`). Scanning for `@username` mentions requires no additional API calls.

The timeline is already fetched during background polling via `trigger_details_fetch()`, which is called from `merge_prs()` whenever `has_changed` is true (comment count change, etc.).

### ReReviewRequested trigger

The GitHub REST timeline endpoint (`/issues/{number}/timeline`) returns `ReviewRequestedEvent` events. The current match in `client.rs` does not handle this type explicitly (falls through to `content: None`). Two additions needed:

1. Add a `reviewer_login: Option<String>` field to `TimelineEvent` (or embed it in `content`) so the requested reviewer's login is available.
2. Add `"ReviewRequestedEvent"` to the match arm in `fetch_timeline()`.

Re-review detection: a `ReviewRequestedEvent` targeting the current user is a *re-request* if a prior `PullRequestReview` by that user already exists in the same timeline. Both event types are available in the timeline response.

### Rate limits

Not a concern. GitHub REST API allows 5,000 authenticated requests/hour. At 20 PRs polling every 5 minutes with timeline fetched only on changed PRs, additional requests stay well under ~300/hour.

### evaluate() signature

```rust
fn evaluate(
    prev_state: Option<&AttentionState>,
    prev_pr: Option<&PullRequest>,
    new_pr: &PullRequest,
    timeline: &[TimelineEvent],
    current_user: &str,
    now: DateTime<Utc>,
    config: &AttentionConfig,
) -> AttentionState
```

First appearance is when `prev_state.is_none()`. Retroactive firing logic is a branch inside `evaluate()`, not a separate call site.

### What replaces needs_attention() and is_unread()

`needs_attention()` in `rules.rs` and `is_unread()` in `lifecycle.rs` are consumed by sort (Priority) and rendering. They should be replaced by methods on `AttentionState`:
- `is_red() -> bool` — any active reason in set
- `is_blue() -> bool` — `updated_at > last_seen_at` and set is empty
- `dot_color() -> Option<DotColor>` — derived from the above

### State persistence

`AttentionState` must be persisted in `state.toml` per PR (keyed by PR id). On load, use `#[serde(default)]` so existing state files without attention fields are treated as first-appearance on next poll. `last_comment_at: Option<DateTime<Utc>>` must be persisted to survive restarts (needed for quiet period logic).

## Implementation Phases

### Phase 1 — Pure domain (`src/domain/attention.rs`)
- [x] Done

- `TriggerReason` enum (9 variants)
- `AttentionState { active_reasons: HashSet<TriggerReason>, last_seen_at, last_comment_at }`
- `evaluate(prev_state, prev_pr, new_pr, current_user, now, config) -> AttentionState` — owns all state transition logic
- Clearing functions per the clearing events table above
- 100% unit-tested; no storage, no UI

### Phase 2 — Wiring + persistence
- [x] Done

- Persist `AttentionState` in `state.toml` per PR
- Wire `evaluate()` into `merge_prs()`
- Hook clearing events into user actions (mark-as-seen, archive, comment posted, etc.)
- E2e tests with mocked provider

### Phase 3 — UI + config
- [x] Done

- Replace `●` / `!` with colored dot driven by `AttentionState`
- Detail view: list active reasons
- Config: quiet period, per-rule toggles, open-in-browser-marks-seen

## Configuration

- **Quiet period** (default 15 minutes, range 0–120 minutes): applies to both `CommentReply` and `NewComments`. Setting to 0 means fire immediately on the first new comment with no settling delay.
- **Per-rule toggles**: each red-dot rule can be individually disabled. A disabled rule never fires and never accumulates silently in the set.
- **Open-in-browser marks as seen** (optional, default off): when enabled, the keypress that opens a PR in the browser marks it as seen immediately, regardless of whether the browser opens successfully.
