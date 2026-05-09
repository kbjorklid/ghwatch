# Architecture Document: ghnotify

## 1. Architectural Style

`ghnotify` adopts a **Modular Monolith** architecture heavily influenced by **Clean Architecture** principles. However, rather than strictly separating code into abstract layers (Domain, Use Cases, Interfaces) which can feel non-idiomatic or overly verbose in Rust, we organize the codebase by **feature slices** or **modules**. 

Within each module, we enforce a strict dependency rule: **Infrastructure depends on Domain, not the other way around.** We use Rust's powerful trait system for dependency inversion where necessary to allow for easier testing and substitution (e.g., mocking the GitHub API).

## 2. High-Level Module Structure

The project is structured into clear, cohesive modules:

```text
src/
├── main.rs            // Entry point, Tokio runtime setup, Dependency Wiring.
├── app.rs             // Global Application State & Main Event Loop.
├── config/            // Configuration management (Settings, TOML parsing).
│   ├── mod.rs         // Config struct, serde definitions.
│   └── watcher.rs     // File watcher for hot-reload (notify crate).
├── domain/            // Core business logic, pure data structures.
│   ├── pr.rs          // PullRequest, PRStatus, ReviewProgress, CIStatus models.
│   ├── rules.rs       // "Needs Attention" deterministic rules engine.
│   ├── lifecycle.rs   // Transitions (Unread, Auto-unfollow logic).
│   └── ports.rs       // Traits (GithubProvider, StateRepository) for DI.
├── github/            // Infrastructure Adapter for GitHub.
│   ├── client.rs      // Struct `GhCliClient` implementing `GithubProvider`.
│   ├── models.rs      // Raw JSON parsing structures (serde).
│   └── rate_limit.rs  // Rate limit tracking, backoff logic.
├── polling/           // Application logic for data fetching.
│   └── worker.rs      // Tokio background tasks, Round-Robin query execution.
├── storage/           // Infrastructure Adapter for Persistence.
│   ├── local.rs       // `FileStateRepository` implementing `StateRepository`.
│   ├── archive.rs     // Archive TOML rotation (1 MB threshold, 3 files max).
│   └── lock.rs        // File locking via `fs2` (single writer, readers).
├── notify/            // Desktop notification dispatcher.
│   └── dispatcher.rs  // `notify-rust` integration, deduplication per PR per cycle.
├── ui/                // Presentation Layer (Ratatui).
│   ├── render.rs      // Main rendering logic (responsive panes).
│   ├── components/    // UI widgets (List, DetailPane, StatusBar, Settings, Archive).
│   ├── search.rs      // Fuzzy filter logic (title, author, repo matching).
│   ├── icons.rs       // Icon helper module (Nerd Font vs plain text fallback).
│   ├── markdown.rs    // `comrak`-based Markdown-to-TUI text rendering.
│   └── events.rs      // `tokio::sync::mpsc` channel definitions (AppEvent).
└── logging.rs         // `tracing` setup, in-memory gh call log.
```

## 3. Dependency Inversion & Traits

To maintain Clean Architecture boundaries, we define traits in the core/domain modules and implement them in the infrastructure modules.

### 3.1. Github Provider

```rust
// In `src/domain/ports.rs`
#[async_trait::async_trait]
pub trait GithubProvider {
    async fn fetch_prs_by_query(&self, query: &str) -> Result<Vec<RawPullRequest>>;
    async fn fetch_pr_details(&self, repo: &str, pr_number: u32) -> Result<RawPullRequestDetails>;
    async fn fetch_check_runs(&self, repo: &str, ref_: &str) -> Result<Vec<CheckRun>>;
    async fn fetch_timeline(&self, repo: &str, pr_number: u32) -> Result<Vec<TimelineEvent>>;
    async fn fetch_rate_limit(&self) -> Result<RateLimitStatus>;
}
```

The concrete implementation `GhCliClient` (in `src/github/client.rs`) will internally use `tokio::process::Command` to execute `gh pr list --json ...` and parse the output.

### 3.2. State Repository

```rust
// In `src/domain/ports.rs`
pub trait StateRepository {
    fn load_state(&self) -> Result<AppState>;
    fn save_state(&self, state: &AppState) -> Result<()>;
    fn load_archive(&self) -> Result<Vec<ArchivedPR>>;
    fn save_archive(&self, archive: &[ArchivedPR]) -> Result<()>;
}
```

The concrete implementation `FileStateRepository` (in `src/storage/local.rs`) uses TOML files with `fs2` file locking.

### 3.3. Notification Dispatcher

```rust
// In `src/domain/ports.rs`
pub trait NotificationDispatcher {
    fn notify(&self, pr: &PullRequest, event: NotificationEvent);
}
```

The concrete implementation (in `src/notify/dispatcher.rs`) uses `notify-rust` and deduplicates: only one notification per PR per change cycle.

## 4. Concurrency Model & Event Loop

The application relies on `tokio` for async operations and `tokio::sync::mpsc` for message passing to the UI thread.

1.  **Main Thread (UI):** 
    *   Initializes the Ratatui terminal.
    *   Listens to an `mpsc::Receiver<AppEvent>`.
    *   Handles rendering and synchronous state updates based on events.
2.  **Input Thread/Task:** 
    *   Reads `crossterm::event` and sends `AppEvent::Input(KeyEvent)` to the UI channel.
3.  **Polling Task (Background):**
    *   A detached `tokio::spawn` task that reads user queries.
    *   Uses a **Round-Robin** approach: on every tick interval (e.g., 30s), it executes *one* query using the `GithubProvider`, then updates its internal pointer. Each query tracks its own `last_polled_at` timestamp and is only executed when its individual `interval` has elapsed.
    *   On fetch completion, it sends `AppEvent::PrsUpdated(Vec<PullRequest>)` to the main UI channel.

### Event Enum Example
```rust
pub enum AppEvent {
    Tick,
    Input(crossterm::event::KeyEvent),
    PrsUpdated { query_name: String, prs: Vec<domain::pr::PullRequest> },
    CiStatusLoaded { repo: String, pr_number: u32, checks: Vec<domain::pr::CheckRun> },
    TimelineLoaded { repo: String, pr_number: u32, events: Vec<domain::pr::TimelineEvent> },
    ConfigReloaded(config::AppConfig),
    Error(String),
}
```

## 5. Domain Logic: "Needs Attention" & Unread State

### 5.1. Unread State
*   **State:** The persistence layer tracks the `last_seen_at` timestamp for each PR.
*   **Action:** When a PR's `updated_at` (from GitHub) is strictly greater than `last_seen_at`, it is marked as `Unread`.
*   **UI Interaction:** The user presses `m` to manually update `last_seen_at` to the current time, clearing the unread delta. Scrolling/viewing does *not* mutate this state.

### 5.2. Needs Attention Logic
Implemented purely as functions in the `domain` module, avoiding external dependencies. 

```rust
// src/domain/rules.rs
pub fn needs_attention(pr: &PullRequest, current_user: &str) -> bool {
    let has_failing_ci = pr.ci_status == CIStatus::Failing;
    let changes_requested = pr.review_status == ReviewStatus::ChangesRequested;
    let pending_review = pr.requested_reviewers.contains(&current_user.to_string());

    has_failing_ci || changes_requested || pending_review
}
```

## 6. UI Layout & Rendering Strategy

*   **Responsiveness:** The UI checks terminal dimensions (`frame.area()`). If width > a threshold (e.g., 120 columns), it splits horizontally (List on Left, Detail on Right). Otherwise, it splits vertically.
*   **Two-Line Layout:** The list view utilizes a custom Ratatui list or table where each item yields two rows of text. This requires custom row calculation but allows massive information density.
*   **Icons:** The UI layer abstracts icons into a helper module `src/ui/icons.rs` which checks a global configuration (`config.use_nerd_fonts`).

## 7. Configuration

*   Using `serde` and `toml`, we load user settings from `~/.config/ghnotify/config.toml`.
*   Configuration dictates:
    *   `queries`: List of query definitions (name, search, interval, enabled) for the round-robin polling.
    *   `unfollow_timeout_mins`: Global timeout for Merged/Closed PRs (default: 60).
    *   `use_nerd_fonts`: Boolean.
    *   `visible_columns`: Array of enums dictating which metrics to show on line 2 of the list.
    *   `theme`: Theme preset name (e.g., "dark", "light").
    *   `show_status_bar`: Boolean.
    *   `notifications`: Per-event-type enable/disable (comments, reviews, ci, commits).

### 7.1. Hot Reload

`config.toml` and `state.toml` are watched using the `notify` crate (file system watcher). On file change:
1.  The watcher task sends `AppEvent::ConfigReloaded(new_config)` to the UI channel.
2.  The main event loop swaps the active config/state and re-renders.
3.  Config validation errors are logged and the previous config is kept.

### 7.2. File Locking (Multi-Instance)

Uses `fs2::FileExt` advisory locks on a lock file (`~/.local/state/ghnotify/.lock`):
1.  On startup, attempt an exclusive lock. If successful, this instance is the **writer**.
2.  If the lock is held, take a shared lock and run as a **reader**.
3.  The writer serializes all state changes. Readers poll the file for updates via the `notify` watcher.

## 8. Rate Limiting & Backoff

*   **Tracking:** `src/github/rate_limit.rs` maintains an `AtomicU32` of remaining API calls. After each `gh api` call, it parses the exit code and output for rate limit info. A dedicated `gh api rate_limit` call is made once per polling cycle to stay accurate.
*   **Backoff:** When `remaining < 100`, the effective poll interval is doubled. When `remaining < 50`, polling pauses entirely and a warning is shown in the status bar. Polling resumes when the limit resets.
*   **Retry:** Failed API calls (non-zero exit, timeout) are retried up to 2 times with exponential backoff (1s, 4s) before surfacing an `AppEvent::Error`.

## 9. Fuzzy Search

*   Pressing `/` activates the search input (rendered as a single-line prompt at the bottom of the screen).
*   As the user types, `src/ui/search.rs` filters the in-memory PR list using a simple substring match against title, author, and repo name. No external fuzzy library — substring matching is sufficient for the expected dataset size (10–50 PRs).
*   The filtered list replaces the main list view until `Esc` clears the filter.

## 10. Desktop Notifications

*   `src/notify/dispatcher.rs` receives change events from the polling worker.
*   **Deduplication:** A `HashMap<PullRequestId, HashSet<NotificationEvent>>` tracks what was already notified in the current cycle. It is cleared at the start of each poll cycle.
*   **Configurable:** Reads `config.notifications` to decide which event types produce desktop notifications.
*   Uses `notify_rust::Notification` to send OS-native notifications.

## 11. Error Handling Strategy

*   **Library errors** (domain, github, storage): Typed enums using `thiserror` (e.g., `GithubError::ApiCallFailed`, `StorageError::Io`).
*   **Application errors** (app, polling): Wrapped in `anyhow::Result` with context.
*   **UI:** Errors are sent as `AppEvent::Error(msg)` and displayed temporarily in the status bar (auto-dismiss after 10s). Fatal errors (e.g., no `gh` CLI) abort startup with a clear message.
*   **Logging:** All errors are logged via `tracing::error!` to the log file.

## 12. Development Roadmap & Phasing
1.  **Phase 1 (Core Model & UI):** Basic TUI layout with responsive panes, static dummy data, navigation (`j`/`k`/`g`/`G`), two-line list rendering, detail pane with Markdown rendering (`comrak`), icon abstraction.
2.  **Phase 2 (GitHub Integration):** Implement `GhCliClient`, execute `gh pr list --json`, parse to Domain models, update UI. On-demand fetching for CI check runs and activity timeline.
3.  **Phase 3 (Polling & Concurrency):** Setup `tokio` background workers, round-robin polling with per-query intervals, `tokio::sync::mpsc` event loop, rate limit tracking and backoff.
4.  **Phase 4 (State & Rules):** Persistence (`directories` + TOML), file locking (`fs2`), Unread marking (`m`/`M`), "Needs Attention" rules engine (3 rules), auto-unfollow with timeout, archive storage with rotation.
5.  **Phase 5 (Features & Polish):** Settings screen (`Shift+S`), fuzzy filter (`/`), desktop notifications (`notify-rust`), hot-reload (`notify` crate), archive view (`Shift+A`), manual follow (`f`), theme selector, column visibility toggling.
