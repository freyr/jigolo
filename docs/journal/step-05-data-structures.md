# Step 5: Collect into a Data Structure

## What I Built

A `SourceRoot` struct with `Display` implementation, `ExitOutcome` enum for typed exit codes, full orchestration logic in `run()`, and integration tests with `assert_cmd`.

## Key Concepts

### `struct` and `#[derive(Debug, Clone)]`

`struct` is Rust's primary way to group related data. All fields are private by default; `pub` makes them accessible outside the module.

`#[derive(Debug)]` enables `{:?}` formatting — essential for development. `#[derive(Clone)]` enables `.clone()` — needed later when the TUI borrows the model immutably for rendering but needs owned copies elsewhere.

### `impl Display` — the `{}` formatter

Implementing `Display` lets you use `{}` in print macros. Key rules:
- Use `writeln!` inside `Display`, not `println!` — you are writing to a formatter, not stdout
- The `?` operator works with `fmt::Result` (propagates `fmt::Error`)
- `path.display()` returns a wrapper that implements `Display`

### `strip_prefix()` for relative paths

Instead of storing a separate `relative_path` field, compute it at display time:
```rust
let relative = file.strip_prefix(&self.path).unwrap_or(file);
```
This eliminates redundant data and the need to keep two fields consistent.

### `ExitOutcome` enum — typed exit codes

The architecture review recommended keeping all `process::exit()` calls in `main()`. The `ExitOutcome` enum makes `run()` fully testable:
```rust
enum ExitOutcome {
    Success,
    AllPathsFailed,
}
```

`run()` returns the *intent*, `main()` translates it to an exit code.

## Alternatives Considered

### 3 structs vs 1 struct

The original plan had `DiscoveredFile`, `SourceRoot`, and `ScanResult`. Simplified to just `SourceRoot`:
- `ScanResult` was a thin wrapper around `Vec<SourceRoot>` — YAGNI
- `DiscoveredFile` stored redundant `relative_path` — computable via `strip_prefix()`

One struct still teaches composition, methods, and trait implementation.

### `process::exit()` in `run()` vs typed return

Calling `process::exit(1)` inside `run()` prevents testing (hidden exits) and prevents cleanup. Returning `ExitOutcome` keeps `run()` pure and centralises exit logic in `main()`.
