# ContextManager

Rust CLI that recursively discovers `CLAUDE.md` files across directory trees.

## Commands

```bash
cargo run                      # Build and run (defaults to current directory)
cargo run -- /path1 /path2     # Scan specific directories
cargo test                     # Run all tests (unit + integration)
cargo clippy -- -D warnings    # Lint with warnings as errors
cargo fmt                      # Format code
```

## Project structure

```
src/main.rs       # All application code (~170 lines)
tests/cli.rs      # Integration tests with assert_cmd
docs/journal/     # Learning journal entries
docs/plans/       # Implementation plans
```

## Conventions

- Single `main.rs` until TUI phase justifies module splitting
- `main()`/`run()` split — `run()` returns `ExitOutcome`, all `process::exit()` calls stay in `main()`
- No `unwrap()` or `expect()` in application code (only in tests)
- All structs derive `Debug` at minimum
- Errors and warnings go to stderr, scan results go to stdout
- Use `filter_entry()` (not `filter()`) to prune directory subtrees
- Use `with_context(|| format!(...))` for lazy error context
- Use `sort_unstable()` for paths
