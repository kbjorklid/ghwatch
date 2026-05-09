# Execution Plan: Phased Implementation

This document describes the phased implementation strategy for ghnotify, designed to fit within LLM context windows by clearing context between each phase.

## Strategy

Each phase is implemented by a **fresh subagent** that:

1. Reads `PRD.md` and `ARCHITECTURE.md` for full product/architecture context.
2. Reads the current codebase to understand what prior phases built.
3. Implements the phase's scope.
4. Returns a summary of what was done.

Phases run **sequentially** — Phase N+1 only starts after Phase N completes. The user can be AFK for the entire run.

## Phases

### Phase 1 — Core Model & UI

Basic TUI layout with responsive panes, static dummy data, navigation (`j`/`k`/`g`/`G`), two-line list rendering, detail pane with Markdown rendering (`comrak`), icon abstraction.

### Phase 2 — GitHub Integration

Implement `GhCliClient`, execute `gh pr list --json`, parse to domain models, update UI. On-demand fetching for CI check runs and activity timeline.

### Phase 3 — Polling & Concurrency

Setup `tokio` background workers, round-robin polling with per-query intervals, `tokio::sync::mpsc` event loop, rate limit tracking and backoff.

### Phase 4 — State & Rules

Persistence (`directories` + TOML), file locking (`fs2`), Unread marking (`m`/`M`), "Needs Attention" rules engine (3 rules), auto-unfollow with timeout, archive storage with rotation.

### Phase 5 — Features & Polish

Settings screen (`Shift+S`), fuzzy filter (`/`), desktop notifications (`notify-rust`), hot-reload (`notify` crate), archive view (`Shift+A`), manual follow (`f`), theme selector, column visibility toggling.

## How to Run

Tell the assistant one of:

- **"Implement all phases"** — runs all 5 phases sequentially, fully autonomous.
- **"Implement Phase N"** — runs a single phase.
- **"Implement Phases N through M"** — runs a range.

Each phase subagent receives a detailed prompt instructing it to read the PRD, ARCHITECTURE, and existing code before writing anything.

### Commit Discipline

After each phase completes successfully (all tests pass, `cargo check` and `cargo clippy` are clean), the subagent **must commit** its work with a descriptive message like `feat(phase N): <summary>`. This ensures progress is never lost and each phase produces a clean checkpoint the next subagent can start from.

## Pre-flight Checklist

Before starting Phase 1, ensure:

- [ ] Rust toolchain installed (MSRV 1.85+, edition 2024)
- [ ] `cargo init` has been run in this directory (or let Phase 1 handle it)
- [ ] `gh` CLI installed and authenticated (needed from Phase 2 onward)

## Status Tracking

| Phase | Status |
|-------|--------|
| 1 — Core Model & UI | Completed |
| 2 — GitHub Integration | Completed |
| 3 — Polling & Concurrency | Completed |
| 4 — State & Rules | Completed |
| 5 — Features & Polish | Not started |
