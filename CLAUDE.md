# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo build          # Build the project
cargo run            # Run the TUI application
cargo check          # Fast compile check (no binary)
cargo clippy         # Lint with warnings
cargo test           # Run all tests
cargo test <name>    # Run a single test by name substring
```

Run `cargo check`, `cargo clippy`, and `cargo test` after any code change. Fix all errors and warnings before finishing.

## Architecture

`ghwatch` is a Ratatui TUI for monitoring GitHub PRs. It uses a modular architecture with a strict dependency rule: **Infrastructure depends on Domain, never the reverse.**

**Core modules:**
- `src/domain/` — pure business logic: PR models (`pr.rs`), "needs attention" rules (`rules.rs`), lifecycle transitions (`lifecycle.rs`), and DI traits (`ports.rs`)
- `src/github/` — `GhCliClient` (implements `GithubProvider` via `gh` CLI subprocess), raw JSON models, rate limit tracking
- `src/polling/` — background Tokio task, round-robin query execution per interval
- `src/storage/` — TOML persistence, `fs2` file locking for multi-instance write safety, archive rotation
- `src/ui/` — Ratatui rendering, `AppEvent` mpsc channel, components for list/detail/settings/archive/diagnostics
- `src/app.rs` — `App` struct holding all state, wires modules together, drives the event loop
- `src/input.rs` — all keyboard/mouse event dispatch, one handler per `AppMode`
- `src/config/` — `AppConfig` (serde/TOML), hot-reload file watcher via `notify` crate

**Concurrency model:**
- Main thread runs the Ratatui render loop, consuming `mpsc::Receiver<AppEvent>`
- Background Tokio task runs polling; sends `AppEvent::PrsUpdated` etc. to the UI channel
- Config/state watchers send `AppEvent::ConfigReloaded` on file change

**Key traits (in `src/domain/ports.rs`):**
- `GithubProvider` — implemented by `GhCliClient`; mock with `mockall` in tests
- `StateRepository` — implemented by `FileStateRepository`
- `NotificationService` — implemented by `NotificationDispatcher`

**Config:** `~/.config/ghwatch/config.toml`. State/lock files in `~/.local/state/ghwatch/`.

## Testing

Follow strict TDD (red-green-refactor). No exceptions.

- Unit tests: `#[cfg(test)] mod tests` in the same file as the code
- Integration/e2e tests: `tests/` directory
- Test naming: `test_<unit>_<scenario>_<expected_result>`
- Mock `GithubProvider` via `mockall` — no real `gh` CLI calls or HTTP in tests
- Use `#[tokio::test]` for async tests

E2e tests (full event-loop with mocked `GithubProvider`) are the highest-priority tests. Domain logic (pure functions) always gets unit tests.

## Code Style

- Do not add comments unless the why is non-obvious
- No extra features or abstractions beyond what the task requires
- `AppMode` enum in `app.rs` drives which input handler is active in `input.rs`
