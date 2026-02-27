# Step 1: Hello World Binary

## What I Built

A minimal Rust binary that prints "ContextManager v0.1.0" using `cargo init --name context-manager`.

## Key Concepts

### Cargo.toml

The project manifest. Key fields:
- `[package]` — name, version, edition (2021 is current)
- `edition = "2021"` — controls which Rust language features are available
- `Cargo.lock` is auto-generated and should be committed for binaries (ensures reproducible builds) but not for libraries (consumers should resolve their own deps)

### `cargo build` vs `cargo run`

- `cargo build` — compiles to `target/debug/context-manager` but does not execute
- `cargo run` — compiles and runs in one step. Most common during development.
- `target/` contains build artefacts, debug symbols, and dependency caches. It can be safely deleted (`cargo clean`), but rebuilds take time.

## Alternatives Considered

### `println!` vs `eprintln!`

| Macro | Output stream | Use when |
|---|---|---|
| `println!` | stdout | Program output (data, results) |
| `eprintln!` | stderr | Errors, warnings, diagnostics |

This matters for piping: `context-manager | grep foo` should only see program output on stdout, not error messages. We use `println!` here because the version string is program output.

### `cargo fmt` vs manual formatting

`cargo fmt` applies the community standard formatting (rustfmt). There is no reason to format manually. Run it after every change — consistency > personal preference.
