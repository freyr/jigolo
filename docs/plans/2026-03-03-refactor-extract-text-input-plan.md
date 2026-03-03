---
title: "refactor: Extract reusable TextInput struct"
type: refactor
date: 2026-03-03
issue: "#25"
---

# Extract Reusable TextInput Struct

## Overview

Extract the duplicated text input handling (Backspace, Left, Right, Char) from three modules into a `TextInput` struct in `src/tui/text_input.rs`. Replace `App.title_input` and `App.title_cursor` with a single `App.text_input: TextInput` field.

## Problem

The same text editing logic is copy-pasted identically across three handlers:

| Module | Method | Enter action | Esc action |
|--------|--------|-------------|------------|
| `files.rs:216` | `handle_title_input_key` | Save snippet | Return to VisualSelect |
| `compose.rs:377` | `handle_export_path_key` | Execute export | Return to Normal |
| `library.rs:165` | `handle_library_rename_key` | Rename snippet | Return to Normal |

The Backspace, Left, Right, and Char(c) arms are byte-for-byte identical (~12 lines each). Only Enter and Esc differ per screen.

## Solution

### New file: `src/tui/text_input.rs`

```rust
#[derive(Debug, Default)]
pub struct TextInput {
    text: String,   // private — cursor invariant protected
    cursor: usize,
}

impl TextInput {
    /// Returns the current text content.
    pub fn text(&self) -> &str { &self.text }

    /// Returns the current cursor position.
    pub fn cursor(&self) -> usize { self.cursor }

    /// Clears text and resets cursor to 0.
    pub fn clear(&mut self) { ... }

    /// Sets text and places cursor at end.
    pub fn set(&mut self, value: &str) { ... }

    /// Handles editing keys (Backspace, Left, Right, Char).
    /// Returns true if the key was consumed.
    pub fn handle_edit_key(&mut self, code: KeyCode) -> bool { ... }
}
```

No action enum. TextInput only owns text editing. Callers own their own Enter/Esc/lifecycle logic.

### Changes to `App` struct

```rust
// Before (app.rs)
pub title_input: String,
pub title_cursor: usize,

// After
pub text_input: TextInput,
```

### Changes to each handler

Each handler shrinks from ~30 lines to ~10 lines. Callers match Enter/Esc themselves:

```rust
// files.rs
pub(crate) fn handle_title_input_key(&mut self, key_event: KeyEvent) {
    match key_event.code {
        KeyCode::Esc => {
            self.text_input.clear();
            self.mode = Mode::VisualSelect;
        }
        KeyCode::Enter => self.save_current_snippet(),
        _ => { self.text_input.handle_edit_key(key_event.code); }
    }
}
```

### Changes to `draw()` in `app.rs`

Replace `self.title_input.as_str()` with `self.text_input.text()` and `self.title_cursor` with `self.text_input.cursor()` in the input bar rendering.

### Changes to `reset_to_normal()` in `app.rs`

Replace `self.title_input.clear(); self.title_cursor = 0;` with `self.text_input.clear();`.

### Other call sites

- `library.rs` rename entry: replace `self.title_input = snippet.title.clone(); self.title_cursor = self.title_input.len();` with `self.text_input.set(&snippet.title);`
- `files.rs` visual select to title input: replace `self.title_input.clear(); self.title_cursor = 0;` with `self.text_input.clear();`
- `files.rs` save_current_snippet_to: replace `self.title_input.trim()` with `self.text_input.text().trim()`
- `compose.rs` export path: replace `self.title_input.trim()` with `self.text_input.text().trim()`

## Acceptance Criteria

- [x] `TextInput` struct with private fields and `handle_edit_key() -> bool` in `src/tui/text_input.rs`
- [x] `App.title_input` and `App.title_cursor` replaced by `App.text_input: TextInput`
- [x] `handle_title_input_key` (files.rs) delegates editing to `TextInput`
- [x] `handle_export_path_key` (compose.rs) delegates editing to `TextInput`
- [x] `handle_library_rename_key` (library.rs) delegates editing to `TextInput`
- [x] Input bar rendering in `draw()` uses `text_input.text()` / `text_input.cursor()`
- [x] `reset_to_normal()` uses `text_input.clear()`
- [x] Unit tests for `TextInput` covering Backspace, Left, Right, Char, cursor at boundaries, clear, set
- [x] All 208 existing tests pass unchanged
- [x] `cargo clippy --all-targets -- -D warnings` clean

## Review Feedback Applied

- Dropped `TextInputAction` enum (simplicity reviewer) — callers match Enter/Esc themselves
- Made fields private with accessors (Kieran) — protects cursor invariant
- Added `set()` method — needed for rename pre-fill in library.rs

## References

- Brainstorm: `docs/brainstorms/2026-03-03-refactoring-review-brainstorm.md`
- Issue: #25
- Duplicated code: `files.rs:226-243`, `compose.rs:387-404`, `library.rs:175-192`
