# ContextManager — Brainstorm

**Date:** 2026-02-27
**Status:** Refined — learning-focused approach defined

---

## What We're Building

A **terminal user interface (TUI) application** for managing Claude Code context files (CLAUDE.md, Memory, configuration). The app provides a dual-pane layout: a navigable file tree on the left and a content viewer/editor on the right.

**Core use case:** Give it one or more paths, it recursively finds all CLAUDE.md files, and lets you browse and edit them in a keyboard-driven interface.

### Target Platforms

- Native on macOS
- Easily cross-compiled for Linux
- Windows not required

---

## Why This Approach

### Language: Rust

**Chosen because:**

- Learning experience is a primary goal — Rust teaches ownership, lifetimes, and systems thinking
- Best-in-class TUI widget ecosystem for this exact use case (tui-textarea, tui-tree-widget)
- Single binary distribution, no runtime dependencies
- Performance headroom for large directory trees
- Natural separation of domain logic into library crates (reusable if a GUI is built later)

**Trade-offs accepted:**

- Steeper learning curve than Go (weeks vs days)
- Cross-compilation to Linux requires the `cross` crate or CI pipeline (not as trivial as Go's `GOOS=linux`)
- Ratatui is community-maintained (but very actively so, 18.6k stars, 18.1M downloads)

### Framework: Ratatui

**With these key crates:**

| Crate | Purpose |
|---|---|
| `ratatui` | Core TUI framework (immediate-mode rendering) |
| `crossterm` | Terminal backend (cross-platform) |
| `tui-tree-widget` | Tree view for file navigation |
| `tui-textarea` | Text editor widget (for later phases) |

### Architecture: Domain/UI Separation

Keep domain logic (file discovery, content loading, path management) in a separate module/crate from the TUI layer. This is not a hard rule — don't over-engineer it — but a natural separation that emerges from Ratatui's immediate-mode architecture where rendering is separate from state.

---

## Key Decisions

1. **Language:** Rust (learning goal + best widget ecosystem)
2. **Framework:** Ratatui + crossterm
3. **MVP scope:** Readonly browser first — tree navigation + file viewing, no editing
4. **Editor complexity (later):** Start basic (navigate, type, delete). No vim mode or syntax highlighting in v1.
5. **Future GUI:** Keep domain logic reasonably separate so it could be reused, but don't let this complicate the code
6. **Development approach:** Straight to Rust — no throwaway prototype in another language

---

## MVP — Readonly Browser

### What's In

- Accept one or more paths as CLI arguments
- Recursively find all `CLAUDE.md` files within those paths
- Left pane: tree view of discovered files (navigable with arrow keys)
- Right pane: display content of selected file (scrollable)
- Keyboard: Up/Down to navigate tree, Enter to select, Tab to switch panes, `q` to quit

### What's Not In (yet)

- File editing (phase 2)
- Autosave (phase 2)
- Word jumping / advanced navigation (phase 2)
- Syntax highlighting (phase 3)
- Mouse support (future)
- Configuration file (future)
- Colour themes (future)

---

## Inspirational Apps

- **Yazi** (Rust) — blazing fast file manager, closest architectural match
- **Joshuto** (Rust/Ratatui) — ranger-like dual-pane file manager
- **Broot** (Rust) — tree-based navigation with fuzzy search
- **Helix** (Rust) — reference for terminal text editing

---

## Design & Prototyping Tools

| Tool | Use |
|---|---|
| ASCIIFlow | Quick wireframe of the dual-pane layout |
| Charm VHS | Scripted demo recordings for README |
| cargo-watch | `cargo watch -x run` for live recompilation during dev |
| Microsoft tui-test / Termwright | E2E testing (when ready) |

---

## Learning Approach

**Primary goal:** Learn Rust through building. The app is the vehicle, Rust mastery is the destination.

### Iteration Strategy: Very Small Steps

Each step teaches **one Rust concept**. The MVP (readonly browser) breaks down into ~10-12 micro-steps, each compilable and runnable:

| Step | Feature | Rust Concepts Learned |
|---|---|---|
| 1 | Hello world binary | Cargo, `main()`, project structure |
| 2 | Parse CLI arguments | `String`, `Vec`, `clap` crate, error types |
| 3 | Read a directory | `std::fs`, `Path`/`PathBuf`, `Result`, error handling |
| 4 | Recursive file search | Recursion, `walkdir` crate, iterators, `filter`/`map` |
| 5 | Collect into a data structure | `struct`, `Vec<T>`, ownership, borrowing basics |
| 6 | Basic TUI with one pane | Ratatui setup, event loop, `crossterm`, terminal raw mode |
| 7 | Render file list in TUI | Ratatui widgets, `List`, rendering pipeline |
| 8 | Keyboard navigation | `match` expressions, enums, state management |
| 9 | Dual-pane layout | Ratatui `Layout`, constraints, splitting areas |
| 10 | Load and display file content | File I/O, `String` vs `&str`, lifetimes (first encounter) |
| 11 | Scroll the content pane | Mutable state, `Viewport`/offset tracking |
| 12 | Tree view (replace flat list) | `tui-tree-widget`, trait implementations, generics |

### Learning Journal

After each step, a journal entry in `docs/journal/` captures:

- **What was built** — the feature added
- **Rust concepts encountered** — with brief explanations
- **Alternatives considered** — 2-3 ways it could have been done, with trade-offs
- **Why this way was chosen** — the idiomatic Rust reasoning
- **What surprised me** — gotchas, compiler errors that taught something
- **Links** — to Rust Book chapters, relevant blog posts, source code of inspirational apps

### Teaching Style

For each significant decision, documentation shows:
1. The **beginner-friendly way** (might compile but isn't idiomatic)
2. The **idiomatic way** (what experienced Rustaceans would write)
3. **Why** the idiomatic way is better (ownership, performance, readability)

This "show alternatives, explain trade-offs" approach builds intuition faster than just showing the "right" answer.

---

## Open Questions

- Should the tree show the full file path or collapse common prefixes?
- What happens when a path contains no CLAUDE.md files? (show empty state? warning?)
- Should the app watch for file system changes and auto-refresh the tree?
- Keyboard shortcut scheme — follow vim conventions (j/k) or arrow-key-first?

---

## Project Structure

```
ContextManager/
  docs/
    brainstorms/          # This file
    journal/              # Learning journal entries (one per step)
  src/                    # Rust source (created by cargo init)
  Cargo.toml
  Outline.md              # Original idea
```

---

## Next Step

Run `/workflows:plan` to create a step-by-step implementation plan for Step 1-3 (pre-TUI foundation).
