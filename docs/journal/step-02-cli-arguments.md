# Step 2: Parse CLI Arguments

## What I Built

CLI argument parsing using `clap` with derive macros. Accepts one or more directory paths, defaulting to `.` when no arguments are provided.

## Key Concepts

### `String` vs `&str` vs `PathBuf` vs `&Path`

| Type | Owned? | UTF-8? | Use for |
|---|---|---|---|
| `String` | Yes | Yes | Text data you own and modify |
| `&str` | No (borrowed) | Yes | Passing text to functions |
| `PathBuf` | Yes | No (OS bytes) | File paths you own (struct fields, Vec elements) |
| `&Path` | No (borrowed) | No (OS bytes) | Function parameters accepting paths |

Rule of thumb: functions should accept `&Path` / `&str` (borrowed), structs should store `PathBuf` / `String` (owned).

### clap Derive Macros

`#[derive(Parser)]` generates argument-parsing code at compile time from struct annotations. The `#[command(version, about)]` attribute pulls version from `Cargo.toml` and the about text from the struct's doc comment.

`#[arg(default_value = ".")]` makes the argument optional with a default. For `Vec<PathBuf>`, user-supplied args **replace** the default entirely — they do not append to it.

### `Vec<T>`

Rust's growable, heap-allocated array. Owns its elements. When a `Vec` is dropped, all elements are dropped. `Vec<PathBuf>` is the natural type for "zero or more paths from the user".

## Alternatives Considered

### clap derive vs clap builder

- **Derive** — annotate a struct, get parsing for free. Concise, type-safe, catches errors at compile time.
- **Builder** — construct parser programmatically at runtime. More flexible for dynamic CLIs.

Derive is recommended for most CLIs. Builder is for advanced cases like plugin systems.

### clap vs `std::env::args()`

`std::env::args()` returns raw strings. You must parse, validate, and generate help text yourself. clap handles all of this. The dependency cost (~21 crates) is justified for any CLI beyond trivial.
