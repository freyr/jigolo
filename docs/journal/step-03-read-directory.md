# Step 3: Read a Single Directory

## What I Built

A `list_directory()` function that reads directory contents using `std::fs::read_dir`, with `anyhow` for error handling. Also introduced the `main()`/`run()` split pattern.

**Note:** This code is scaffolding — Step 4 replaces `list_directory()` with `walkdir`-based recursive traversal. The purpose here is learning `Result`, `?`, and `anyhow`.

## Key Concepts

### `Result<T, E>` and the `?` operator

Rust's primary error handling mechanism. Functions return `Result<T, E>` — either `Ok(value)` or `Err(error)`. The `?` operator propagates errors to the caller automatically, equivalent to an early return on error.

```rust
let entries = fs::read_dir(path)?; // if Err, return early with the error
```

### The `main()`/`run()` split

Returning `Result` from `main()` directly:
- Always exits with code 1 on error
- Prints errors using Debug format (ugly)
- Cannot distinguish between exit codes

The split gives control:
```rust
fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {:#}", err); // {:#} = full chain, one line
        process::exit(1);
    }
}
```

### `anyhow` for application error handling

`anyhow::Result<T>` wraps any error type. `with_context()` adds human-readable context:
```rust
fs::read_dir(path)
    .with_context(|| format!("Failed to read directory: {}", path.display()))?;
```

Use `with_context(|| ...)` (closure) not `context(format!(...))` — the closure is only evaluated on error.

## Alternatives Considered

### `unwrap()` vs `?` vs `match`

| Approach | Behaviour on error | Use when |
|---|---|---|
| `unwrap()` | Panics (crashes) | Tests only. Never in application code. |
| `?` | Propagates to caller | Functions returning `Result` — the default choice |
| `match` | Explicit branching | Need different behaviour per error kind |

### `anyhow` vs `std::io::Error`

Plain `io::Error` says "No such file or directory" — but for which file? `anyhow` with `with_context` produces "Failed to read directory: /nonexistent: No such file or directory". The context makes errors actionable.
