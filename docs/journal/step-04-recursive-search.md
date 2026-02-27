# Step 4: Recursive File Search

## What I Built

A `find_claude_files()` function that recursively walks directory trees using `walkdir`, filtering for files named exactly `CLAUDE.md`. Errors are logged to stderr and skipped. Results are sorted alphabetically.

## Key Concepts

### Iterators

Rust's most powerful abstraction. Iterator chains (`.filter()`, `.map()`, `.collect()`) are lazy — nothing executes until `.collect()` materialises the results. The compiler optimises chains into a single loop (zero-cost abstraction).

### `filter_entry()` vs `filter()` — the critical distinction

This is the single most important performance insight:
- `filter_entry()` **prunes entire subtrees**. If `node_modules` is filtered, none of its children are visited.
- `filter()` still **descends into** filtered directories — it just skips individual entries.
- On a home directory with JS projects, this is the difference between <1 second and 30-60 seconds.

### Closures

`|entry| entry.ok()` is a closure — an anonymous function. Rust infers types. Closures can capture variables from their environment, but the ones used here do not need to.

### `OsStr` comparison

`entry.file_name() == "CLAUDE.md"` works because `OsStr` implements `PartialEq<str>`. Rust's trait system enables cross-type comparisons ergonomically.

## Alternatives Considered

### Iterator chain vs explicit for loop

```rust
// Beginner way — explicit loop
let mut files = Vec::new();
for entry in WalkDir::new(root) {
    if let Ok(entry) = entry {
        if entry.file_type().is_file() && entry.file_name() == "CLAUDE.md" {
            files.push(entry.into_path());
        }
    }
}

// Idiomatic way — iterator chain
let files: Vec<PathBuf> = WalkDir::new(root)
    .into_iter()
    .filter_map(|e| e.ok())
    .filter(|e| e.file_type().is_file())
    .filter(|e| e.file_name() == "CLAUDE.md")
    .map(|e| e.into_path())
    .collect();
```

Both compile to essentially the same machine code. The iterator version is more idiomatic — each step is self-documenting.

### `walkdir` vs `ignore` vs manual recursion

- **walkdir** — simple, single-threaded, well-tested. Right choice for learning.
- **ignore** — respects `.gitignore`, supports parallelism. Better for production tools scanning large trees.
- **Manual recursion** — educational but error-prone (must handle symlink cycles yourself).

### `sort_unstable()` vs `sort()`

`sort_unstable()` is slightly faster and uses less memory. Use it when equal elements have no meaningful ordering distinction — true for file paths.
