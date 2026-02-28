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

## References

- [Feature plan](docs/plans/2026-02-27-feat-readonly-browser-foundation-plan.md)
- [Brainstorm](docs/brainstorms/2026-02-27-context-manager-brainstorm.md)
- [Learning journal](docs/journal/)
