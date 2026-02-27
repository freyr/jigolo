# Rust Concepts Used in ContextManager

Core Rust concepts encountered during Steps 1-5, organised by the order they appear in the code.

## Ownership and Types

Rust has no garbage collector. Every value has exactly one **owner**, and the value is dropped (freed) when the owner goes out of scope.

```rust
let path = PathBuf::from("/tmp");  // `path` owns this heap-allocated path
let files = vec![path];            // ownership moves into the Vec — `path` is no longer usable
```

The two path types reflect this:
- **`PathBuf`** — owned, heap-allocated. Use in structs and return values.
- **`&Path`** — borrowed reference. Use in function parameters. Equivalent to `&str` vs `String` for text.

## Structs and `#[derive]`

Structs group related data. No inheritance — Rust uses composition and traits instead.

```rust
#[derive(Debug, Clone)]
struct SourceRoot {
    path: PathBuf,
    files: Vec<PathBuf>,
}
```

`#[derive(...)]` auto-generates trait implementations at compile time:
- `Debug` — enables `{:?}` formatting for printing during development
- `Clone` — enables `.clone()` to duplicate the value

## `impl` Blocks

Methods are defined in `impl` blocks, separate from the struct definition:

```rust
impl SourceRoot {
    fn file_count(&self) -> usize {  // &self = borrow the struct immutably
        self.files.len()
    }
}
```

`&self` means the method borrows the struct without taking ownership. The caller can keep using it after the call.

## Traits — `Display`

Traits are Rust's version of interfaces. `Display` lets a type be formatted with `{}`:

```rust
impl fmt::Display for SourceRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.path.display())?;  // write to the formatter, not stdout
        Ok(())
    }
}
```

Key detail: inside `Display`, you use `write!`/`writeln!` (writes to a formatter), not `println!` (writes to stdout).

## `Result<T, E>` and the `?` Operator

Rust has no exceptions. Functions that can fail return `Result<T, E>` — either `Ok(value)` or `Err(error)`:

```rust
fn list_directory(path: &Path) -> Result<()> {
    let entries = fs::read_dir(path)?;  // ? = if Err, return early with that error
    // ...
    Ok(())
}
```

`?` is syntactic sugar for "unwrap if Ok, return the error if Err". It replaces verbose `match` blocks.

## Enums

Enums can hold different variants, optionally with data. Used here for typed exit codes:

```rust
enum ExitOutcome {
    Success,
    AllPathsFailed,
}
```

Pattern matching with `match` is exhaustive — the compiler ensures you handle every variant:

```rust
match run() {
    ExitOutcome::Success => {}
    ExitOutcome::AllPathsFailed => process::exit(1),
}
```

## Iterators and Closures

Iterator chains are Rust's most powerful abstraction. They are lazy (nothing runs until `.collect()`) and zero-cost (the compiler optimises them into a single loop):

```rust
let files: Vec<PathBuf> = WalkDir::new(root)
    .into_iter()
    .filter_entry(should_descend)       // prune subtrees
    .filter_map(|result| result.ok())   // skip errors
    .filter(|e| e.file_type().is_file())
    .filter(|e| e.file_name() == "CLAUDE.md")
    .map(|e| e.into_path())            // DirEntry -> PathBuf
    .collect();                          // materialise into Vec
```

Each `|e|` is a **closure** — an anonymous function. Rust infers the types.

Key distinction used in this project:
- `.filter_entry()` — **prunes entire subtrees** (never descends into `node_modules`)
- `.filter()` — skips entries but still descends into their children

## The `main()`/`run()` Split

A Rust-specific CLI pattern. `main()` cannot return custom exit codes if it returns `Result`. The split separates logic from process control:

```rust
fn run() -> ExitOutcome {
    // all logic here — returns a value, no process::exit()
}

fn main() {
    match run() {
        ExitOutcome::Success => {}
        ExitOutcome::AllPathsFailed => process::exit(1),
    }
}
```

This makes `run()` testable without spawning a subprocess.

## `Vec<T>` — The Growable Array

`Vec` is heap-allocated, owns its elements, and frees them when dropped:

```rust
let mut files: Vec<PathBuf> = Vec::new();
files.push(PathBuf::from("/tmp/CLAUDE.md"));
files.sort_unstable();  // sort in-place; unstable = slightly faster, fine for paths
```

When a `Vec` is dropped, every element inside it is dropped too. No manual cleanup needed.
