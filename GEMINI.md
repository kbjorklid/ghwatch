# AGENTS.md

## General Instructions

- Read `PRD.md` and `ARCHITECTURE.md` before writing any code.
- Read the existing codebase to understand what prior phases have built.
- Follow the phasing strategy described in `EXECUTION_PLAN.md`.
- Use the module structure defined in `ARCHITECTURE.md` §2.
- Follow the dependency rule: Infrastructure depends on Domain, never the reverse.
- Do not add comments to code unless explicitly requested.
- Run `cargo check`, `cargo clippy`, and `cargo test` after each phase. Fix all errors and warnings before proceeding.

## Test-Driven Development (TDD)

All production code must be written using strict **red-green-refactor** TDD. No exceptions.

### Bug Fixes

When a bug is reported or discovered, you MUST:
1. **Red — Write a failing test.** Create a test case that reproduces the bug. Run the test to confirm it fails.
2. **Green — Fix the bug.** Implement the fix so the test passes.
3. **Refactor — Clean up.** Ensure the fix is idiomatic and doesn't introduce regressions.

### Red-Green-Refactor Cycle

For every unit of behavior, follow this exact sequence:

1. **Red — Write a failing test.**
   - Write a test that describes the desired behavior.
   - Run `cargo test`. The test MUST fail. If it passes, the test is wrong.
   - Commit the failing test (if the user has asked for commits).

2. **Green — Write the minimum code to make the test pass.**
   - Write the simplest possible implementation that makes the failing test pass.
   - Do not add extra features, abstractions, or edge-case handling beyond what the test requires.
   - Run `cargo test`. All tests must pass.

3. **Refactor — Clean up without changing behavior.**
   - Remove duplication, improve naming, reorganize modules.
   - Run `cargo test` after each refactor step. All tests must still pass.
   - Do not change behavior during refactoring.

### Strict TDD Compliance

As a senior engineer agent, you are held to the highest standard of TDD discipline. You MUST:
1. **Declare the Phase:** Before every code modification (test or production code), explicitly state which TDD phase you are in (**RED**, **GREEN**, or **REFACTOR**) in your reasoning or topic update.
2. **Never Skip RED:** You are strictly forbidden from writing production code before a failing test exists. If a test passes on the first try, you must explain why (e.g., existing coverage) or refine the test to be more specific.
3. **Surgical GREEN:** Implement only what is required to pass. "Future-proofing" or "just-in-case" logic is a violation of the **GREEN** phase.
4. **Verified REFACTOR:** Only refactor when all tests are passing. Run tests after every discrete refactoring step.

### Test Organization

- Unit tests live in the same file as the code they test, inside a `#[cfg(test)] mod tests` block.
- Integration tests live in the `tests/` directory.
- Test names must be descriptive: `test_<unit>_<scenario>_<expected_result>`.

### Testing Priorities

Testing effort should prioritize **end-to-end behavior** — user-facing workflows, event flows, and feature integrations — over isolated unit tests. Unit tests are still required for pure domain logic, but the primary confidence comes from e2e tests that exercise the full stack (minus external services).

- **E2e tests (highest priority):** Full user workflows through the TUI event loop — navigating the list, marking PRs as read, filtering, grouping, triggering attention rules. These tests wire up real modules end-to-end, replacing only the `GithubProvider` trait with a mock. They live in the `tests/` directory.
- **Domain logic (always TDD):** Rules, lifecycle transitions, state changes — pure functions with no I/O. Unit tests in `#[cfg(test)] mod tests`.
- **Parsing and data transformation — always TDD.**
- **UI rendering:** Snapshot or component tests where practical.
- **Infrastructure adapters:** Test against the trait interface using mocks only. No real `gh` CLI calls, no HTTP, no fixtures.

### GitHub API Mocking Strategy

Use **trait-based mocks only**. The `GithubProvider` trait in `domain/ports.rs` is the seam — all tests mock it via `mockall`. This keeps tests fast, deterministic, and decoupled from `gh` CLI output format.

- Do not record or replay real `gh` CLI responses.
- Do not stand up fake HTTP servers.
- Real `gh` integration is validated manually or in a future CI step, not in automated tests.

### Test Dependencies

- Use `mockall` for trait mocking.
- Use `tokio::test` for async tests.
- Keep tests independent. No shared mutable state between tests.
- Test names must be descriptive: `test_<unit>_<scenario>_<expected_result>`.

## Ad-hoc Testing Environment

- **Restricted Repository:** For any manual or ad-hoc testing involving the `gh` CLI or real GitHub API interactions, you MUST ONLY use the repository `kbjorklid/gh-notify-test`.
- **No Other Repositories:** You are STRICTLY FORBIDDEN from interacting with any other GitHub repository during testing or development activities. This ensures that personal or production data is never touched or modified.
- **Cleanup:** Always clean up any test data (PRs, issues, comments) created in `kbjorklid/gh-notify-test` after testing is complete.
