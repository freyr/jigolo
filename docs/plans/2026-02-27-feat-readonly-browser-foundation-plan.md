---
title: "ContextManager — Readonly Browser Foundation (Steps 1–5)"
type: feat
date: 2026-02-27
brainstorm: docs/brainstorms/2026-02-27-jigolo-brainstorm.md
---

# ContextManager — Readonly Browser Foundation (Steps 1–5)

## Changes from Original Plan

- Added `main()`/`run()` split pattern — required for the plan's own exit code scheme
- Simplified data model from 3 structs to 1 (`SourceRoot` only; `ScanResult` and `DiscoveredFile` were YAGNI)
- Added `filter_entry()` for directory pruning — without it, scanning `~` takes 30-60 seconds on machines with JS projects
- Replaced silent error swallowing with stderr logging to match design decisions
- Added `#[derive(Debug, Clone)]` to all structs
- Added testing strategy with `assert_cmd` + `tempfile`
- Resolved contradiction: permission denied now prints warning (was "silently skipped" in acceptance criteria)
- Resolved contradiction: deferred path canonicalisation (was "canonicalise at input time" but no code implemented it)
- Step 3 explicitly marked as scaffolding replaced by Step 4

---

## Overview

Build the pre-TUI foundation for ContextManager: a Rust CLI that accepts directory paths, recursively discovers all `CLAUDE.md` files, and collects them into a structured data model. This covers the first 5 micro-steps of the learning plan, each teaching one core Rust concept.

**Primary goal:** Learn Rust. The app is the vehicle.

**Teaching approach:** Each step includes alternatives considered with trade-offs explained — showing the beginner way, the idiomatic way, and why.

## Design Decisions (Resolving Open Questions)

These decisions close the critical gaps identified during spec analysis. Each is deliberately simple — we can revisit as the app matures.

| Decision | Choice | Rationale |
|---|---|---|
| No arguments provided | Default to current directory (`.`) | Unix convention (e.g. `ls`, `find`). Least surprising. |
| Non-existent path | Print warning to stderr, skip it, continue | Partial results are more useful than aborting entirely. Exit 1 only if *all* paths fail. |
| Permission denied | Skip directory, print warning to stderr | Same rationale — don't abort for partial failures. Errors are logged via `eprintln!`, not silently dropped. |
| No CLAUDE.md found | Empty result, exit 0 | "No results" is valid; not an error. Print "No CLAUDE.md files found." to stdout. |
| Symlinks | Follow them (`follow_links(true)`) | Simple. Cycle detection is handled by walkdir. Note: walkdir's default is `follow_links(false)` — we explicitly enable it. |
| Multiple paths | Group by source root | `Vec<SourceRoot>` where each root holds its discovered files. Preserves the user's input grouping for later TUI tree display. |
| File matching | Exact case-sensitive `CLAUDE.md` | Standard on macOS/Linux. Deliberately byte-exact — this is the secure choice against confusable-character attacks. |
| Result ordering | Alphabetical by path within each root; roots in argument order | Deterministic, predictable. `sort_unstable()` is slightly faster than `sort()` and appropriate here since paths have no meaningful stability requirement. |
| Path normalisation | **Deferred** — use paths as provided | `canonicalize()` adds complexity (fails on non-existent paths, creates TOCTOU gaps). Use user-provided paths for display. Revisit when TUI needs stable node identity. |
| Data structure | `Vec<SourceRoot>` with nested `Vec<PathBuf>` | Simple, ordered, maps naturally to a tree view later. |
| Exit codes | 0 = success (even if empty), 1 = all paths failed, 2 = bad arguments (handled by clap automatically) | Standard Unix convention. Exit code 2 requires no implementation — clap calls `std::process::exit(2)` on parse errors by default. |
| Directory filtering | Skip `.git`, `node_modules`, `target`, and other build artifact dirs via `filter_entry()` | Without this, scanning real developer directories is unusably slow. `filter_entry()` prunes entire subtrees; `filter()` does not. |
| Path argument is a file | Treat as failed path — warn on stderr, skip | Users might accidentally pass `/path/to/CLAUDE.md` instead of `/path/to/`. `walkdir` on a file yields just that file; detect and warn. |
| Duplicate paths | Not deduplicated — process each as given | Simplest approach. User's intent is respected. May produce duplicate output; acceptable for Steps 1-5. |
| Overlapping paths | Not deduplicated | Files under child path may appear in both roots' results. Documented as known limitation. |
| stdout vs stderr | Scan results → stdout. Warnings/errors → stderr. Progress messages ("Scanning N directories...") → stderr | Allows piping stdout without pollution from progress/warning messages. |
| Maximum depth | No limit by default, but capped at 100 as a safety valve | Prevents accidental traversal of entire filesystem. |

---

## Implementation Phases

### Step 1: Hello World Binary

**Rust concepts:** Cargo, `main()`, project structure, `cargo run`, `cargo fmt`, `cargo clippy`.

#### What to build

A minimal Rust binary that prints "ContextManager v0.1.0" and exits. **Note:** This hardcoded version is intentionally temporary — Step 2 replaces it with Cargo.toml-derived versioning via clap.

#### Tasks

1. Install Rust via rustup (if not already installed)
2. Run `cargo init --name jigolo` inside the ContextManager directory
3. Edit `src/main.rs` to print the app name and version
4. Run `cargo run` to verify it works
5. Run `cargo fmt` and `cargo clippy` — get in the habit

#### Expected `src/main.rs`

```rust
fn main() {
    println!("ContextManager v0.1.0");
}
```

#### Learning journal entry topics

- What `Cargo.toml` contains and what each field means
- What `cargo build` vs `cargo run` does
- What the `target/` directory contains
- Why `Cargo.lock` should be committed for binary projects (not for libraries)
- **Alternative:** `println!` vs `eprintln!` — when to use each (stdout vs stderr)

#### Files created/modified

- `Cargo.toml` (generated by `cargo init`)
- `src/main.rs`
- `docs/journal/step-01-hello-world.md`

---

### Step 2: Parse CLI Arguments

**Rust concepts:** `String`, `Vec<T>`, `PathBuf`, the `clap` crate with derive macros, `#[derive()]`, basic error types.

#### What to build

Accept one or more directory paths as CLI arguments. Default to `.` if none provided. Print the parsed paths.

#### Tasks

1. Add `clap` dependency: `cargo add clap --features derive`
2. Define a `Cli` struct with `#[derive(Parser)]`
3. Accept `paths: Vec<PathBuf>` with a default of `.`
4. Print parsed paths to verify
5. Run `cargo run -- /some/path /another/path` to test
6. Run `cargo run` with no args to verify default
7. Add a configuration sanity test (see Gotchas below)

#### Expected CLI behaviour

```bash
# No args — defaults to current directory
$ cargo run
Searching in: ["."]

# Single path
$ cargo run -- /Users/michal/.claude
Searching in: ["/Users/michal/.claude"]

# Multiple paths
$ cargo run -- /path1 /path2
Searching in: ["/path1", "/path2"]

# Help
$ cargo run -- --help
A TUI for managing Claude Code context files

Usage: jigolo [PATHS]...

Arguments:
  [PATHS]...  Directories to search for CLAUDE.md files [default: .]

Options:
  -h, --help     Print help
  -V, --version  Print version
```

#### Key code

```rust
use std::path::PathBuf;

use clap::Parser;

/// A TUI for managing Claude Code context files
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Directories to search for CLAUDE.md files
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    println!("Searching in: {:?}", cli.paths);
}
```

#### Learning journal entry topics

- `String` vs `&str` vs `PathBuf` vs `&Path` — the four common text/path types and when to use each
- **Alternative: clap derive vs clap builder** — derive is more concise, builder gives more control. Derive is recommended for beginners.
- **Alternative: clap vs argh vs std::env::args()** — why clap is the community standard despite being larger
- What `#[derive(Parser)]` does under the hood (proc macros generate code at compile time)
- `Vec<T>` — Rust's growable array, heap-allocated, owns its elements

#### Gotchas

**`Vec<PathBuf>` default behaviour:** When `default_value = "."` is set and the user passes arguments, the default is **not** included — user args replace it entirely. Also, `default_value = ""` does not work for `PathBuf` ([clap-rs/clap#5368](https://github.com/clap-rs/clap/issues/5368)).

**Always include this test** to catch clap configuration errors at compile time:
```rust
#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
}
```

#### Files created/modified

- `Cargo.toml` (add clap dependency)
- `src/main.rs`
- `docs/journal/step-02-cli-arguments.md`

---

### Step 3: Read a Single Directory

**Rust concepts:** `std::fs`, `Path`/`PathBuf`, `Result<T, E>`, the `?` operator, `anyhow` for error handling, `match` expressions.

**Note:** This step is scaffolding — Step 4 replaces `list_directory()` entirely with `walkdir`-based recursive traversal. The value of this step is teaching `Result`, `?`, `anyhow`, and `Path` vs `PathBuf` in isolation before combining them with iterators.

#### What to build

Read the contents of a single directory and list its entries. Handle errors gracefully (non-existent paths, permission denied).

#### Tasks

1. Add `anyhow` dependency: `cargo add anyhow`
2. Introduce the `main()`/`run()` split pattern (see below)
3. Write a function that takes a `&Path` and returns `Result<Vec<DirEntry>>`
4. Iterate entries and print file names, indicating files vs directories
5. Test with valid paths, non-existent paths, and permission-denied scenarios

#### Key code

```rust
use std::fs;
use std::path::Path;
use std::process;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;

fn list_directory(path: &Path) -> Result<()> {
    let entries = fs::read_dir(path)
        .with_context(|| format!("Failed to read directory: {}", path.display()))?;

    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let prefix = if file_type.is_dir() { "d" } else { "f" };
        println!("  [{}] {}", prefix, entry.file_name().to_string_lossy());
    }

    Ok(())
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    for path in &cli.paths {
        list_directory(path)?;
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {:#}", err);
        process::exit(1);
    }
}
```

#### Learning journal entry topics

- **`Result<T, E>` and the `?` operator** — Rust's primary error handling mechanism. `?` propagates errors up the call stack. Compare with try/catch in other languages.
- **Alternative: `unwrap()` vs `?` vs `match`**
  - `unwrap()` — panics on error. Never use in application code. Fine in tests.
  - `?` — propagates error to caller. Idiomatic for functions that return `Result`.
  - `match` — explicit handling. Use when you need different behaviour for different errors.
- **`anyhow` vs plain `std::io::Error`** — anyhow wraps any error type and adds context strings. Plain `io::Error` loses context about *what* failed.
- **`Path` vs `PathBuf`** — like `&str` vs `String`. `Path` is borrowed (a reference), `PathBuf` is owned (on the heap). Functions should accept `&Path` for maximum flexibility.
- **`to_string_lossy()`** — file names on Unix are bytes, not guaranteed UTF-8. `to_string_lossy()` replaces invalid bytes with `?`. Alternative: `to_str()` which returns `Option<&str>`. Note: this returns `Cow<'_, str>` (Copy on Write) — a good time to briefly introduce `Cow`.
- **Ownership moment:** `entry` inside the loop owns the `DirEntry`. When the loop iteration ends, it's dropped. This is Rust's RAII in action.

#### The `main()`/`run()` split pattern

This is the most important Rust CLI pattern for this project. Returning `Result` from `main()` directly always exits with code 1 and prints errors using `Debug` (ugly). The split gives you control over exit codes and error formatting:

```rust
fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {:#}", err);  // {:#} shows the full error chain on one line
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    // Actual logic here — can use ? freely
}
```

This matters because the plan requires exit codes 0, 1, and 2 — impossible with `main() -> Result<()>`. The `run()` function is also testable without subprocess spawning.

**Tip:** Use `with_context(|| format!(...))` instead of `context(format!(...))` — the closure is only evaluated on error.

#### Files created/modified

- `Cargo.toml` (add anyhow dependency)
- `src/main.rs`
- `docs/journal/step-03-read-directory.md`

---

### Step 4: Recursive File Search

**Rust concepts:** External crates (`walkdir`), iterators, `.filter()`, `.map()`, `.collect()`, closures, iterator chaining.

#### What to build

Recursively walk a directory tree and find all files named exactly `CLAUDE.md`. Print each match with its full path. Skip known-unproductive directories (`.git`, `node_modules`, `target`) to avoid catastrophic performance.

#### Tasks

1. Add `walkdir` dependency: `cargo add walkdir`
2. Write a function `find_claude_files(root: &Path) -> Vec<PathBuf>` (returns `Vec`, not `Result` — see below)
3. Use `WalkDir` with `filter_entry()` to prune unproductive directories
4. Use iterator chaining to filter for `CLAUDE.md` files
5. Log skipped errors to stderr (not silent drop)
6. Sort results alphabetically with `sort_unstable()`
7. Test with real Claude directories (`~/.claude/`, any project with CLAUDE.md)

#### Key code

```rust
use std::path::Path;
use std::path::PathBuf;

use walkdir::DirEntry;
use walkdir::WalkDir;

/// Directories that will never contain CLAUDE.md files.
/// Using `filter_entry()` prunes entire subtrees — this is the critical
/// performance optimisation. Without it, scanning a home directory with
/// JS projects can take 30-60 seconds instead of <1 second.
const SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", "target", ".cache",
    "__pycache__", ".venv", "vendor", "dist",
    ".next", ".nuxt", "build",
];

fn should_descend(entry: &DirEntry) -> bool {
    if entry.file_type().is_dir() {
        let name = entry.file_name().to_string_lossy();
        return !SKIP_DIRS.iter().any(|d| *d == name.as_ref());
    }
    true
}

fn find_claude_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(root)
        .follow_links(true)
        .max_depth(100)
        .into_iter()
        .filter_entry(should_descend)    // Prunes entire subtrees — critical!
        .filter_map(|result| match result {
            Ok(entry) => Some(entry),
            Err(err) => {
                eprintln!(
                    "Warning: {}: {}",
                    err.path().map(|p| p.display().to_string())
                        .unwrap_or_else(|| "<unknown>".into()),
                    err
                );
                None
            }
        })
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == "CLAUDE.md")
        .map(|entry| entry.into_path())
        .collect();

    files.sort_unstable();
    files
}
```

#### Learning journal entry topics

- **Iterators** — Rust's most powerful abstraction. Lazy evaluation (nothing happens until `.collect()`). Zero-cost abstraction — the compiler optimises iterator chains into a single loop.
- **Alternative: iterator chain vs for loop**
  ```rust
  // Beginner way — explicit for loop with mut vec
  let mut files = Vec::new();
  for entry in WalkDir::new(root) {
      if let Ok(entry) = entry {
          if entry.file_type().is_file()
              && entry.file_name() == "CLAUDE.md"
          {
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
  Both compile to essentially the same machine code. The iterator version is more idiomatic because each step is self-documenting and composable.
- **`filter_entry()` vs `filter()` — critical performance distinction**
  - `filter_entry()` **prevents descent** into filtered directories. If `.git` is filtered, none of its children are visited.
  - `filter()` still **descends** into filtered directories — it just skips individual entries after visiting them.
  - This is the difference between 0.1-second and 30-second scans on real developer machines.
- **`filter_map(|e| e.ok())`** — combines filter + map. Converts `Result<T, E>` to `Option<T>`, keeping only `Ok` values. This is the idiomatic way to skip errors, but in our case we log them first.
- **Closures** — `|entry| entry.ok()` is a closure (anonymous function). Rust infers the types. Closures can capture variables from their environment (but these don't need to).
- **`entry.file_name() == "CLAUDE.md"`** — this works because `OsStr` implements `PartialEq<str>`. Rust's trait system makes cross-type comparison ergonomic.
- **`.inspect()` for debugging iterator chains** — insert `.inspect(|e| eprintln!("Considering: {:?}", e.path()))` anywhere in the chain to see what is being processed. Remove before committing.
- **`sort_unstable()` vs `sort()`** — `sort_unstable()` is slightly faster and uses less memory. Use it when there is no meaningful distinction between equal elements (true for paths).
- **Alternative: `walkdir` vs `ignore` vs manual recursion**
  - `walkdir` — simple, single-threaded, well-tested. Best for learning.
  - `ignore` — respects `.gitignore`, parallel. Overkill here but useful to know exists.
  - Manual recursion with `fs::read_dir` — educational but error-prone (you must handle symlink cycles yourself).
  - **Exercise:** Try both. Walk `~/code` with `walkdir` (no `filter_entry` pruning), then with the `ignore` crate. Measure wall-clock time. This teaches more about practical performance than any textbook.

#### Why `find_claude_files` returns `Vec<PathBuf>` not `Result`

The function can never return `Err` — all walkdir errors are handled inline (logged and skipped). A `Result` return type is misleading when it can only be `Ok`. Root validation (exists? is directory?) happens in the caller before invoking this function.

**Note:** Avoid calling `entry.metadata()` in the iterator chain — it makes a `stat()` syscall. `file_type()` and `file_name()` are cached and free.

#### Files created/modified

- `Cargo.toml` (add walkdir dependency)
- `src/main.rs`
- `docs/journal/step-04-recursive-search.md`

---

### Step 5: Collect into a Data Structure

**Rust concepts:** `struct` definitions, `Vec<T>`, ownership vs borrowing, `impl` blocks, methods, the `Display` trait, `#[derive()]` traits.

#### What to build

Define proper data types for the app's domain model. Replace ad-hoc `Vec<PathBuf>` with a meaningful `SourceRoot` struct. Process multiple root paths, group results, and implement exit code logic.

#### Tasks

1. Define `SourceRoot` struct (holds root path + discovered file paths)
2. Add `#[derive(Debug, Clone)]` to the struct
3. Implement methods on `SourceRoot` (e.g., `file_count()`)
4. Implement `Display` for pretty-printing results
5. Write the orchestration logic in `run()`: parse args → scan each root → collect results → determine exit code → print summary
6. Run `cargo clippy` and address all warnings
7. Add integration tests with `assert_cmd` (see Testing Strategy section)

#### Key data structures

```rust
use std::fmt;
use std::path::PathBuf;

/// One of the root directories provided by the user, with all CLAUDE.md files found within it.
#[derive(Debug, Clone)]
pub struct SourceRoot {
    /// The root directory path (as provided by the user)
    pub path: PathBuf,
    /// Full paths to all discovered CLAUDE.md files within this root
    pub files: Vec<PathBuf>,
}

impl SourceRoot {
    /// Number of CLAUDE.md files found in this root
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

impl fmt::Display for SourceRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.file_count();
        let label = if count == 1 { "file" } else { "files" };
        writeln!(f, "{} ({} {})", self.path.display(), count, label)?;
        for file in &self.files {
            // Compute relative path at display time via strip_prefix
            let relative = file
                .strip_prefix(&self.path)
                .unwrap_or(file);
            writeln!(f, "  {}", relative.display())?;
        }
        Ok(())
    }
}
```

#### Orchestration logic with exit codes

```rust
use std::process;

use clap::Parser;

/// Return value from run() — keeps all process::exit() calls in main().
enum ExitOutcome {
    Success,
    AllPathsFailed,
}

fn run() -> ExitOutcome {
    let cli = Cli::parse();

    let mut roots: Vec<SourceRoot> = Vec::new();
    let mut failed_count: usize = 0;

    eprintln!("Scanning {} directories...", cli.paths.len());

    for path in &cli.paths {
        if !path.exists() {
            eprintln!("Warning: path does not exist: {}", path.display());
            failed_count += 1;
            continue;
        }
        if !path.is_dir() {
            eprintln!("Warning: not a directory: {}", path.display());
            failed_count += 1;
            continue;
        }

        let files = find_claude_files(path);
        roots.push(SourceRoot {
            path: path.clone(),
            files,
        });
    }

    if roots.is_empty() && failed_count > 0 {
        return ExitOutcome::AllPathsFailed;
    }

    // Print results
    let total: usize = roots.iter().map(|r| r.file_count()).sum();

    if total == 0 {
        println!("No CLAUDE.md files found.");
    } else {
        for root in &roots {
            println!();
            print!("{}", root); // Uses Display impl
        }
        println!(
            "Found {} CLAUDE.md {} in {} {}.",
            total,
            if total == 1 { "file" } else { "files" },
            roots.len(),
            if roots.len() == 1 { "directory" } else { "directories" }
        );
    }

    ExitOutcome::Success
}

fn main() {
    match run() {
        ExitOutcome::Success => {}
        ExitOutcome::AllPathsFailed => process::exit(1),
    }
}
```

#### Expected output

```bash
$ cargo run -- ~/.claude ~/code/my-project
Scanning 2 directories...

/Users/michal/.claude (3 files)
  CLAUDE.md
  projects/my-project/CLAUDE.md
  projects/other/CLAUDE.md

/Users/michal/code/my-project (1 file)
  CLAUDE.md

Found 4 CLAUDE.md files in 2 directories.
```

#### Learning journal entry topics

- **`struct`** — Rust's primary way to group related data. No inheritance (use composition and traits instead). All fields are private by default; `pub` makes them accessible outside the module.
- **`#[derive(Debug, Clone)]`** — derive `Debug` on every struct for `{:?}` formatting during development. Derive `Clone` when you need to duplicate the data (required for TUI rendering where you clone data for display while the model is borrowed elsewhere). Consider `PartialEq, Eq` for testing with `assert_eq!`.
- **`impl` blocks** — where you define methods. `&self` borrows the struct immutably, `&mut self` borrows mutably, `self` takes ownership (consumes the struct).
- **Alternative: struct with methods vs tuple struct vs plain tuple**
  ```rust
  // Plain tuple — unclear what each element means
  let file: (PathBuf, PathBuf) = (full_path, relative_path);

  // Tuple struct — named but still positional access
  struct DiscoveredFile(PathBuf, PathBuf);
  let file = DiscoveredFile(full_path, relative_path);

  // Named struct — self-documenting, the idiomatic choice
  struct DiscoveredFile { path: PathBuf, relative_path: PathBuf }
  ```
  Named structs are almost always preferred because they make code self-documenting.
- **`impl Display`** — implementing the `Display` trait lets you use `{}` in `println!`. Key points:
  - Use `writeln!` (not `println!`) inside `Display` — `Display` writes to a formatter, not directly to stdout
  - The `?` operator works with `fmt::Result` too (propagates `fmt::Error`)
  - `path.display()` returns a `Display`-implementing wrapper for `Path`
- **Ownership in structs** — `SourceRoot` *owns* its `Vec<PathBuf>`. When a `SourceRoot` is dropped, all its paths are dropped too. This is Rust's memory management — no garbage collector needed.
- **`iter().map().sum()`** — another iterator chain. `.sum()` works because `usize` implements `Sum`. This replaces the need for a manual accumulator loop.

#### Why one struct instead of three

The original plan had `DiscoveredFile`, `SourceRoot`, and `ScanResult`. Simplified to just `SourceRoot`:
- `ScanResult` was a thin wrapper around `Vec<SourceRoot>` — use the `Vec` directly
- `DiscoveredFile` stored both `path` and `relative_path`, but `relative_path` is derivable via `strip_prefix()` at display time — no need to store it

At ~200 lines, keep everything in `main.rs`. Module splitting is motivated naturally by Step 6's TUI code.

#### Files created/modified

- `src/main.rs`
- `docs/journal/step-05-data-structures.md`

---

## Testing Strategy

Testing is a cross-cutting concern with specific activities for Steps 4 and 5.

**Core testing crates ([Rust CLI Book](https://rust-cli.github.io/book/tutorial/testing.html)):**

| Crate | Purpose | Add as |
|---|---|---|
| `assert_cmd` | Run CLI binary and assert stdout/stderr/exit codes | `[dev-dependencies]` |
| `predicates` | Flexible assertion matchers (contains, regex, etc.) | `[dev-dependencies]` |
| `tempfile` | Create temporary directories for test fixtures | `[dev-dependencies]` |

Add these: `cargo add --dev assert_cmd predicates tempfile`

**Unit test example (Step 4 — `find_claude_files`):**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn finds_claude_md_in_nested_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create nested structure
        fs::create_dir_all(root.join("sub/deep")).unwrap();
        fs::write(root.join("CLAUDE.md"), "root").unwrap();
        fs::write(root.join("sub/CLAUDE.md"), "sub").unwrap();
        fs::write(root.join("sub/deep/CLAUDE.md"), "deep").unwrap();
        fs::write(root.join("sub/not-claude.md"), "ignored").unwrap();

        let files = find_claude_files(root);

        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| f.file_name().unwrap() == "CLAUDE.md"));
    }

    #[test]
    fn returns_empty_for_no_claude_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("README.md"), "not claude").unwrap();

        let files = find_claude_files(tmp.path());

        assert!(files.is_empty());
    }

    #[test]
    fn skips_filtered_directories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("node_modules/deep")).unwrap();
        fs::write(root.join("node_modules/deep/CLAUDE.md"), "skip").unwrap();
        fs::write(root.join("CLAUDE.md"), "keep").unwrap();

        let files = find_claude_files(root);

        assert_eq!(files.len(), 1);
    }
}
```

**Integration test example (Step 5 — full CLI):**

```rust
// tests/cli.rs
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn no_args_searches_current_directory() {
    Command::cargo_bin("jigolo")
        .unwrap()
        .assert()
        .success();
}

#[test]
fn nonexistent_path_warns_on_stderr() {
    Command::cargo_bin("jigolo")
        .unwrap()
        .arg("/nonexistent/path/that/does/not/exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Warning"));
}

#[test]
fn finds_claude_md_in_temp_dir() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), "test").unwrap();

    Command::cargo_bin("jigolo")
        .unwrap()
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("1 file"));
}

#[test]
fn help_flag_succeeds() {
    Command::cargo_bin("jigolo")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("CLAUDE.md"));
}
```

---

## Acceptance Criteria

### Functional Requirements

- [x] `cargo run` with no args searches current directory for CLAUDE.md files
- [x] `cargo run -- /path1 /path2` searches both paths
- [x] `cargo run -- --help` prints usage information (exit code 0)
- [x] `cargo run -- --version` prints version from Cargo.toml (exit code 0)
- [x] Non-existent paths produce a warning on stderr but don't abort
- [x] Permission-denied directories produce a warning on stderr and are skipped
- [x] Symlink cycles produce a warning on stderr and are skipped
- [x] Symlinks are followed (`follow_links(true)`)
- [x] Results are sorted alphabetically within each root
- [x] Roots are printed in argument order
- [x] Output shows grouped results with file counts per root (singular/plural)
- [x] "No CLAUDE.md files found." message when no files discovered
- [x] Exit code 0 on success (even if no files found)
- [x] Exit code 1 if all paths fail (non-existent or not a directory)
- [x] Exit code 2 on bad arguments (handled automatically by clap)
- [x] Directories `.git`, `node_modules`, `target` are pruned via `filter_entry()`
- [x] File path arguments (not directories) produce a warning and are skipped

### Testing Requirements

- [x] `cargo test` passes
- [x] Unit tests for `find_claude_files()` (happy path, empty result, filtered directories)
- [x] Integration tests with `assert_cmd` (no args, non-existent path, help flag)

### Learning Requirements

- [x] Each step has a learning journal entry in `docs/journal/`
- [x] Each journal entry documents at least 2 alternatives with trade-offs
- [x] Code compiles without warnings after each step (`cargo clippy` clean)
- [x] Code is formatted (`cargo fmt`)

### Quality Gates

- [x] `cargo clippy -- -D warnings` passes (all warnings treated as errors)
- [x] `cargo fmt --check` passes
- [x] `cargo test` passes
- [x] No `unwrap()` or `expect()` in application code (only in tests)
- [x] All structs derive `Debug` at minimum

---

## Dependencies & Prerequisites

| Dependency | Version | Purpose |
|---|---|---|
| Rust (via rustup) | stable (latest) | Language toolchain |
| `clap` | 4.x | CLI argument parsing (derive feature) |
| `walkdir` | 2.x | Recursive directory traversal |
| `anyhow` | 1.x | Application error handling |

**Dev dependencies (for testing):**

| Dependency | Version | Purpose |
|---|---|---|
| `assert_cmd` | 2.x | CLI integration testing |
| `predicates` | 3.x | Flexible test assertions |
| `tempfile` | 3.x | Temporary directories for test fixtures |

**Total dependency footprint:** 3 direct + 3 dev dependencies. Deliberately minimal.

---

## Development Workflow

### For each step

1. **Read** the step description and understand the Rust concept
2. **Write** the code (start from the examples, modify as needed)
3. **Run** `cargo run` to verify it works, then `cargo test`
4. **Run** `cargo clippy` and `cargo fmt` to clean up
5. **Commit** with a descriptive message referencing the step
6. **Write** the learning journal entry while the experience is fresh

### Useful commands

```bash
cargo run                      # Build and run
cargo run -- /some/path        # Run with arguments
cargo test                     # Run all tests
cargo clippy                   # Lint — follow every suggestion
cargo fmt                      # Auto-format
cargo doc --open               # View docs for your dependencies
cargo add <crate>              # Add a dependency
```

---

## References

### Rust Learning

- [The Rust Programming Language (The Book)](https://doc.rust-lang.org/book/) — chapters 1-5 cover everything in Steps 1-5
- [Rust By Example](https://doc.rust-lang.org/rust-by-example/) — practical examples for each concept
- [Comprehensive Rust (Google)](https://comprehensive-rust.mo8it.com/) — fast-paced alternative to The Book
- [Command Line Applications in Rust (Rust CLI Book)](https://rust-cli.github.io/book/) — the official guide for CLI development

### Crate Documentation

- [clap docs](https://docs.rs/clap) — CLI parsing ([derive reference](https://docs.rs/clap/latest/clap/_derive/index.html))
- [walkdir docs](https://docs.rs/walkdir) — directory traversal
- [anyhow docs](https://docs.rs/anyhow) — error handling
- [assert_cmd docs](https://docs.rs/assert_cmd) — CLI testing

### Inspirational Source Code

- [ripgrep](https://github.com/BurntSushi/ripgrep) — workspace layout with library crates, by the author of `walkdir`
- [Joshuto](https://github.com/kamiyaa/joshuto) — Rust/Ratatui file manager
- [Broot](https://github.com/Canop/broot) — Rust tree-based file navigator
- [Yazi](https://github.com/sxyazi/yazi) — Rust file manager

---

## What Comes After Step 5

Steps 6-12 (the TUI phase) will be planned separately once the foundation is solid. The data structures from Step 5 feed directly into the TUI:

- `Vec<SourceRoot>` → populates the tree view (left pane)
- `SourceRoot.files[n]` → loads content for the viewer (right pane)
- `SourceRoot` → top-level nodes in the tree

At that point, consider:
- Splitting `main.rs` into modules (`model.rs`, `scanner.rs`) — motivated by TUI code needing separation
- Introducing `lib.rs` for testability
- Switching from `walkdir` to `ignore` for `.gitignore`-aware traversal
- Adding content reading and lazy loading
- Proper signal handling for terminal restoration on Ctrl+C
