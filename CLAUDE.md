# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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

## Architecture

Rust CLI + TUI that recursively discovers `CLAUDE.md` files across directory trees. Two modes: `--list` prints results to stdout and exits; default mode launches a dual-pane ratatui TUI browser.

**Module layout:**

- `src/main.rs` — CLI entry point. `main()` calls `run()` which returns `ExitOutcome`; all `process::exit()` calls stay in `main()`. Orchestrates discovery, then either prints list output or launches TUI.
- `src/model.rs` — Data types: `Cli` (clap args), `SourceRoot` (root path + discovered files), `ExitOutcome` enum.
- `src/discovery.rs` — File discovery: `find_claude_files()` walks a directory tree using walkdir with `filter_entry()` to prune `SKIP_DIRS` (node_modules, .git, target, etc.). `find_global_claude_file()` checks `$HOME/.claude/CLAUDE.md`.
- `src/tui/app.rs` — TUI application using ratatui + tui-tree-widget. Dual-pane layout: left tree (30%) with file navigation, right pane (70%) with file content and scrollbar. Vim-style keybindings (hjkl), Tab to switch panes.
- `src/lib.rs` — Re-exports modules as pub for integration test access.
- `tests/cli.rs` — Integration tests using `assert_cmd` and `predicates`. All tests use `--list` mode to avoid TUI.

**Key data flow:** CLI args → `find_claude_files()` per path → optionally prepend global CLAUDE.md → `--list` prints or TUI renders `Vec<SourceRoot>` as a tree.

## Conventions

- `main()`/`run()` split — `run()` returns `ExitOutcome`, all `process::exit()` calls stay in `main()`
- No `unwrap()` or `expect()` in application code (only in tests)
- All structs derive `Debug` at minimum
- Errors and warnings go to stderr, scan results go to stdout
- Use `filter_entry()` (not `filter()`) to prune directory subtrees
- Use `with_context(|| format!(...))` for lazy error context
- Use `sort_unstable()` for paths
- Each `use` statement imports a single item (no `{}` grouping)
