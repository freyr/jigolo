# Jigolo

Rust CLI + TUI that recursively discovers `CLAUDE.md` files across directory trees. Uses ratatui for a dual-pane browser with vim-style navigation.

## Commands

```bash
cargo run                      # Build and run TUI (defaults to current directory)
cargo run -- /path1 /path2     # Scan specific directories
cargo run -- --list /path      # List mode: print files and exit (no TUI)
cargo test                     # Run all tests (unit + integration)
cargo test test_name           # Run a single test by name
cargo clippy -- -D warnings    # Lint with warnings as errors
cargo fmt                      # Format code
```

## Code Verification

Always use the `rust-cargo` MCP tool for code verification. Run `cargo-clippy` (with `all_targets` and `warnings_as_errors` enabled) as part of every verification pass.

## Critical Rules

- No `unwrap()` or `expect()` in application code (only in tests)
- Each `use` statement imports a single item (no `{}` grouping)
- `main()`/`run()` split — `run()` returns `ExitOutcome`, all `process::exit()` calls stay in `main()`
- All structs derive `Debug` at minimum
- Errors and warnings go to stderr, scan results go to stdout
- Use `filter_entry()` (not `filter()`) to prune directory subtrees
- Use `with_context(|| format!(...))` for lazy error context
- Use `sort_unstable()` for paths

## Architecture

- `src/main.rs` — CLI entry point, orchestrates discovery then prints or launches TUI
- `src/model.rs` — Data types: `Cli`, `SourceRoot`, `ExitOutcome`
- `src/discovery.rs` — File discovery using walkdir with `filter_entry()` to prune `SKIP_DIRS`
- `src/tui/app.rs` — Dual-pane TUI: left tree (30%) + right content pane (70%), vim keybindings (hjkl, Tab)
- `src/lib.rs` — Re-exports modules for integration test access
- `tests/cli.rs` — Integration tests using `assert_cmd`, all via `--list` mode

**Data flow:** CLI args → `find_claude_files()` per path → optionally prepend global CLAUDE.md → `--list` prints or TUI renders `Vec<SourceRoot>`

## Boundaries

**NEVER:**
- Use `unwrap()`/`expect()` outside tests
- Call `process::exit()` outside `main()`
- Group imports with `{}` in `use` statements

**ASK FIRST:**
- New dependencies in Cargo.toml
- Changes to the `SKIP_DIRS` list
- Architectural changes to the module layout

## Rust Conventions

- Use `cargo check` instead of `cargo build` for fast verification during development.
- Error handling: `thiserror` v2 for domain/library errors, `anyhow` for application-level code.
- No `.unwrap()` in library code; use `expect("reason")` only when panic is truly acceptable.
- Before every commit run: `cargo fmt`, then `cargo clippy --all-targets -- -D warnings`, then `cargo test`.
- Add `#![deny(clippy::all)]` at crate root. Add `#![warn(clippy::pedantic)]` only after comfortable with baseline warnings.
- Prefer iterators over manual index loops.
- Prefer `if let` / `match` over `is_some()` + `unwrap()`.
- Use `let...else` for early returns from fallible patterns.
- CLI projects use the clap derive + thiserror + anyhow pattern (see `/rust-cli-scaffold`).
- Test organization: unit tests in `#[cfg(test)] mod tests` inline, integration tests in `tests/`.
- All Rust implementation work follows TDD — write failing test first, then implement.
- Write doc comments on all public items (summary line in third person, then details).
- Use Context7 to look up crate documentation when uncertain about APIs.

### Beginner Anti-Patterns to Flag in Reviews
1. Unnecessary `.clone()` to satisfy borrow checker — restructure ownership instead.
2. `.unwrap()` in non-test code — use `?` with `Result`.
3. Manual index loops (`for i in 0..len`) — use iterators.
4. `is_some()` + `unwrap()` — use `if let Some(val)`.
5. Sentinel values (`-1`, `""`) — use `Option<T>`.
6. Overuse of `Rc<RefCell<T>>` — redesign data ownership.
7. Initialize-then-assign pattern — use constructors.
8. Overly long iterator chains (>3 steps) — break into named functions.
9. Non-exhaustive match with wildcard catch-all — use explicit arms.
10. Missing `.context()` on `?` at module boundaries — always add context.

## Workflow

- Every code change (feature, fix, improvement) MUST have a corresponding GitHub Issue created **before** work begins.
- Use `gh issue create` to create issues. Label them appropriately: `enhancement` for features, `bug` for fixes, `improvement` for refactors/improvements.
- Reference the issue number in commit messages (e.g., `feat: add search mode (#12)`).
- Branch names should include the issue number (e.g., `12-add-search-mode`).

## References

- [Feature plan](docs/plans/2026-02-27-feat-readonly-browser-foundation-plan.md)
- [Brainstorm](docs/brainstorms/2026-02-27-context-manager-brainstorm.md)
- [Learning journal](docs/journal/)
