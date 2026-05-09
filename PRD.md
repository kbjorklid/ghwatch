# Product Requirement Document (PRD): ghnotify

## 1. Overview

`ghnotify` is a Terminal User Interface (TUI) application built with Rust and Ratatui, designed for monitoring GitHub Pull Requests (PRs). It targets tech leads and developers in team/enterprise settings who need to track 10–50 PRs simultaneously across multiple repositories.

The core functionality revolves around a dynamically updated, groupable list of watched PRs with instant detail previews, smart "needs attention" detection, and desktop notifications — all driven by the `gh` CLI.

**Platform:** github.com only. Online-only (no offline mode). Requires `gh` CLI installed and authenticated.

## 2. User Interface (TUI)

The interface is designed for high information density with vim-style modal navigation, full mouse support, and a responsive layout.

### 2.1. Layout

1. **Main List View:** A scrollable, sortable, groupable list displaying all currently watched PRs using a two-line layout per item.
2. **Detail Pane (Responsive):** Updates instantly as the cursor moves. Shows description, reviewer list, diff stats, CI status, and activity timeline. Layout switches between side-by-side and top-and-bottom based on terminal width.
3. **Status Bar (Hideable):** Displays countdown to next refresh and context-sensitive keybindings. Can be hidden from settings to save screen space.

### 2.2. Main List — Two-Line Layout

* **Line 1 (Core Context):** ID, Title (truncated with `…`), Status (color-coded).
* **Line 2 (Metadata & Engagement):** Author, Age/Staleness, Diff Size (+Additions / -Deletions), Review Progress, Comment Count.

**Grouping:** Configurable collapsible sections. Supported dimensions: repo, author, status (open/closed/merged), my-vs-other. Groups display as collapsible section headers inline in the list. The "my-vs-other" split requires knowing the user's GitHub identity (see §3.3).

**Sorting:** Press `s` to cycle sort modes (updated, created, priority, repo).

**Search/Filter:** Press `/` to open a live fuzzy filter prompt. Filters list by title, author, and repo as you type. Escape clears.

**Customization:** Users can configure visible columns and layout preferences via the in-app Settings screen (see §8), persisted to `config.toml`.

**Visual Representation:** Metric indicators use Nerd Font icons (e.g., `✓ 2/2` for approvals, `💬 3` for comments), with a toggleable fallback to standard text abbreviations.

**Theming:** Theme selector with built-in presets for light and dark terminal backgrounds. User picks from the Settings screen.

### 2.3. Detail Pane

A single, continuous, scrollable view combining:

1. **PR Description** — rendered from Markdown using the `comrak` crate (CommonMark-to-TUI text rendering, wrapping long lines, preserving heading/list formatting).
2. **Reviewer List** — reviewers and their statuses (approved, changes requested, pending).
3. **Diff Stats** — additions/deletions summary.
4. **CI Status** — GitHub Actions check run names and conclusions (fetched on-demand when the PR is highlighted, not during polling). Shows check name + status only (e.g., `✗ test-suite: failure`). User presses `o` to open GitHub for full logs.
5. **Activity Timeline** — fetched from the GitHub Timeline API (`/repos/{owner}/{repo}/issues/{number}/timeline`). Shows comments, reviews, label changes, and commits.

The pane updates dynamically on cursor movement. Moving the cursor does **not** clear unread status.

## 3. Smart Following & Unfollowing

### 3.1. Auto-Follow Queries

Users define **named queries** in config, each with a GitHub search string, a configurable poll interval, and an optional group tag.

Example:
```toml
[[query]]
name = "My PRs"
search = "is:pr author:@me state:open"
interval = "60s"
enabled = true

[[query]]
name = "Review Queue"
search = "is:pr review-requested:@me state:open"
interval = "120s"
enabled = true

# Disabled example shipped on first run
[[query]]
name = "Team PRs"
search = "is:pr org:my-org state:open"
interval = "300s"
enabled = false
```

**Polling Strategy:** Round-robin. Queries are cycled one-per-tick to distribute API requests. Each query has its own interval.

**First Run:** Ships with disabled example queries. User enables/customizes them in Settings or by editing `config.toml` directly (hot-reloaded).

### 3.2. Manual Follow

Users can track specific PRs by:
* Pasting a full GitHub URL (`https://github.com/owner/repo/pull/123`)
* Entering shorthand (`owner/repo#123`)

**UI:** Press `f` to open a single-line input prompt at the bottom of the screen (similar to the `/` fuzzy filter prompt). Type or paste the URL/shorthand and press `Enter` to follow. `Esc` cancels.

### 3.3. GitHub Identity

To support the "my PRs vs others" grouping:
* **Primary:** Username in `config.toml`.
* **Fallback:** Auto-detected from `gh api user` at startup.
* **First run:** Auto-detect, pre-fill in config, zero-config launch (no interactive wizard).

### 3.4. Unfollowing & Lifecycle

1. **Auto-Unfollow:** Triggered when a PR reaches a terminal state (Merged, Closed). A **global configurable timeout** applies uniformly (e.g., 1 hour) before removal from the active view.
2. **Manual Unfollow:** Press `u` to explicitly remove.
3. **Archive:** A separate full-screen archive view (opened with `Shift+A`) for browsing and searching previously followed PRs. Archive data is stored in a rotating TOML file: `archive.toml` → `archive.1.toml` → `archive.2.toml`. Rotation triggers when `archive.toml` exceeds 1 MB. A maximum of 3 archive files are kept (total ~3 MB). The oldest file is deleted on rotation.

## 4. Unread State & Notifications

### 4.1. Tracking Changes

1. **Mark as Seen:** Press `m` to mark a single PR as seen. Press `M` to mark all visible PRs as seen. Cursor navigation does **not** clear unread status.
2. **Delta Highlighting:** PRs with new comments, reviews, status changes, or commits since last "seen" are visually highlighted (bold or yellow).

### 4.2. Desktop Notifications

* **Platform:** `notify-rust` crate (supports Linux via dbus, macOS, and Windows via winrt).
* **Trigger Events:** New comments, new reviews, CI pass/fail, new commits.
* **Deduplication:** Only one notification per PR per change cycle (not per poll).
* **Configurable:** Can be enabled/disabled per event type from Settings.

## 5. "Needs Attention" Logic

PRs requiring immediate action are visually distinct inline in the normal sort order.

### 5.1. MVP Rules (Deterministic)

A PR enters "needs attention" state when any of:
1. **Changes Requested** — someone requested changes on your PR.
2. **CI Failed** — GitHub Actions checks failed on your PR.
3. **Pending Review** — you are requested as a reviewer and haven't responded.

### 5.2. Visuals

Inline in the standard list (not floated to top). Differentiated with bold inverted colors or striking visual styling.

### 5.3. Dismissal

Auto-dismissed when the triggering condition clears (e.g., you submit the review, CI passes). No manual dismiss needed.

### 5.4. Future (v2)

LLM-based sentiment and urgency analysis. Explicitly deferred.

## 6. Technical Architecture

### 6.1. Stack

| Component | Choice |
|-----------|--------|
| Language | Rust 2024 edition (MSRV 1.85+) |
| TUI | Ratatui + custom widgets (no separate `ratatui-widgets` crate) |
| Async runtime | Tokio |
| GitHub API | Shell out to `gh api` / `gh pr list --json` via `tokio::process::Command` |
| Auth | Delegated to `gh` CLI (user manages `gh auth`) |
| Error handling | `anyhow` (app) + `thiserror` (library error enums) |
| Persistence | TOML via `directories` crate (XDG dirs) |
| Logging | `tracing` to file + in-memory `gh` call log |
| Notifications | `notify-rust` |
| Markdown rendering | `comrak` (CommonMark parsing for detail pane) |
| File locking | `fs2` crate (single writer, multiple readers) |

### 6.2. Concurrency Model

1. **Async Runtime:** `tokio` manages background polling, keeping the UI responsive.
2. **Event Loop:** Channel-based architecture (`mpsc`) communicates `Tick` and `DataUpdate` events between background workers and the rendering loop.
3. **API Calls:** Each query poll shells out to `gh api` asynchronously. Results are parsed from JSON and dispatched to the UI thread.
4. **Rate Limit Tracking:** Run `gh api` with `--jq` to include response metadata, or parse `x-ratelimit-remaining` via a dedicated `gh api rate_limit` call (executed once per polling cycle). Proactively back off by doubling the effective poll interval when remaining requests drop below 100.

### 6.3. Data Fetching Strategy

| Data | When Fetched | API Call |
|------|-------------|----------|
| PR core fields (title, author, status, diff stats, reviewers) | Every poll cycle | `gh pr list --json` (batch per query) |
| CI check runs | On highlight | `gh api repos/{owner}/{repo}/commits/{ref}/check-runs` |
| Activity timeline | On highlight | `gh api repos/{owner}/{repo}/issues/{number}/timeline` |

### 6.4. Persistence & File Layout

Three TOML files in XDG directories:

| File | Path (Linux) | Purpose |
|------|-------------|---------|
| `config.toml` | `~/.config/ghnotify/config.toml` | User preferences, queries, display settings, theme |
| `state.toml` | `~/.local/state/ghnotify/state.toml` | Active followed PRs, seen status, last-known data |
| `archive.toml` | `~/.local/state/ghnotify/archive.toml` | Completed PRs history (logrotate-style rotation) |

**Hot Reload:** `config.toml` and `state.toml` are watched for file changes. Modifications apply immediately without restart.

**Multi-Instance:** A file lock ensures only one instance writes at a time. Additional instances run as read-only readers that see state changes from the writer.

### 6.5. Logging & Debugging

* **File logging:** `tracing` crate writes to `~/.local/state/ghnotify/ghnotify.log` with daily rotation.
* **In-memory `gh` call log:** Every `gh api` invocation is recorded with timestamp, command, exit code, and duration. Viewable from Settings as a diagnostic screen.

## 7. Keybindings (MVP — Fixed)

| Key | Action |
|-----|--------|
| `j` / `k` | Move cursor down / up |
| `g` / `G` | Jump to top / bottom |
| `s` | Cycle sort mode |
| `Ctrl+g` | Cycle group-by dimension |
| `f` | Follow a PR (open URL/shorthand input) |
| `/` | Open fuzzy filter |
| `m` | Mark current PR as seen |
| `M` | Mark all visible PRs as seen |
| `u` | Unfollow current PR |
| `o` | Open current PR in browser |
| `Tab` | Toggle detail pane focus |
| `Shift+A` | Open archive view |
| `Shift+S` | Open settings screen |
| `?` | Open help overlay |
| `Esc` | Close overlay / clear filter |
| `q` | Quit |

**Help Discovery:** Hideable status bar at bottom shows context-sensitive bindings. `?` opens full help overlay.

**Mouse:** Click to select, scroll to navigate, drag pane borders to resize.

## 8. In-App Settings Screen

A full-screen overlay (opened with `Shift+S`) covering:

1. **Query Management** — Add, edit, remove, enable/disable auto-follow queries. Round-trips to `config.toml`.
2. **Column Visibility** — Toggle which fields appear in the two-line layout.
3. **Display Preferences** — Nerd font toggle, theme selector, status bar visibility.
4. **Polling** — Adjust global and per-query poll intervals.

All changes persist to `config.toml` and hot-reload in any running instance.

## 9. Installation & Distribution

* **Primary:** `cargo install ghnotify`
* **Prerequisites:** Rust 1.85+, `gh` CLI installed and authenticated.

## 10. Scope Summary

Everything in this document is **v0.1 (MVP)** scope. Explicitly **deferred** to future versions:

* LLM-based attention analysis (v2)
* User-defined custom attention rules
* Keybinding customization
* GitHub Enterprise Server support
* Offline mode
* In-app GitHub actions (approve, comment, request changes)
