---
title: "feat: Snippet compose mode (Phase 2)"
type: feat
date: 2026-03-02
issue: "#1"
status: waiting (blocked by #19 — app.rs split)
---

# Snippet Compose Mode (Phase 2)

## Overview

Add a third TUI screen (`[3 Compose]`) where users assemble new CLAUDE.md files from their snippet library. Users toggle-select snippets with Space, preview the composed output, and export to a file.

This completes Phase 2 of the snippet library feature. Phase 1 (capture + browse) shipped in v0.1.0.

## Prerequisites

- #19 (Split app.rs into per-screen modules) — must land first so Compose gets its own clean module

## Problem Statement

Users can save interesting CLAUDE.md sections to their snippet library, but there is no way to compose those snippets into a new file. The library is currently write-only from a workflow perspective — you can add, browse, rename, and delete, but you cannot assemble output from it.

## Proposed Solution

A new `Screen::Compose` variant accessible via key `3`, with a single-pane snippet list and a preview toggle.

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Access method | New screen via `3` key | Compose is a distinct workflow, not a sub-mode of Files; matches Settings as a peer screen |
| Selection UX | Space toggles checkbox | Familiar pattern (file managers, todo apps); simple to implement with `HashSet<usize>` |
| Snippet separator | Two newlines (`\n\n`) hardcoded | Standard Markdown paragraph separator; no need to parameterize for MVP |
| Preview = export | WYSIWYG | What users see in preview is exactly what gets written to disk |
| Compose state | In-memory across tab switches | Selection preserved when switching to Files/Settings and back; lost on app exit |
| `q` behavior | Quits app (consistent with Files/Settings) | `Esc` returns to Files screen; `q` exits app — matches established pattern |
| Layout | Single pane list + preview toggle | Simpler than dual-pane; `p` shows full-screen preview, `Esc` returns to list |
| Library ownership | Borrow from `App.library`, don't clone | Avoids staleness; invalidate `ComposeState` on library mutation |

### Deferred to follow-up issues

| Feature | Reason |
|---------|--------|
| Reorder (`J`/`K`) | Premature — users can compose in library order; add if requested |
| Clipboard (`y` + `arboard`) | New dependency; file export is sufficient for MVP |
| Select all / deselect all (`a`/`A`) | YAGNI — toggle individually with Space |
| `create_dir_all` for missing parents | Masks typos; show error instead |
| Overwrite confirmation sub-mode | Refuse overwrite with status message; simpler than a sub-mode |

## Technical Approach

### New Types

```rust
// src/tui/compose.rs — new module (after #19 app.rs split)

#[derive(Debug)]
pub struct ComposeState {
    /// Set of selected (checked) snippet indices (into App.library)
    pub selected: HashSet<usize>,
    /// Cursor position in the snippet list
    pub cursor: usize,
    /// Scroll offset for the list
    pub scroll: usize,
    /// Viewport height (set during draw)
    pub viewport_height: usize,
}
```

No `ComposePaneFocus` enum — single pane with preview toggle.
No `order: Vec<usize>` — compose in library order.
No cloned `SnippetLibrary` — borrow from `App.library`.

### Mode addition

```rust
pub enum Mode {
    // ... existing variants ...
    ExportPath,  // NEW — text input for export file path (named for intent, not mechanism)
}
```

### Compose Logic

```rust
// src/compose.rs — pure function, no TUI dependency

/// Concatenates selected snippets in library order, separated by double newlines.
pub fn compose_snippets(snippets: &[Snippet], selected: &HashSet<usize>) -> String
```

Single function. Separator hardcoded. No `order` parameter.

### Keybindings (Compose Screen)

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate snippet list |
| `Space` | Toggle selection |
| `p` | Show full-screen composed preview (scrollable, `Esc` returns to list) |
| `w` | Enter ExportPath mode (type file path, `Enter` to write) |
| `Esc` | Return to Files screen |
| `q` | Quit app |
| `1` / `2` | Switch to Files/Settings |

### File Export Flow

1. User presses `w` → mode switches to `Mode::ExportPath`, cursor appears in an input bar at the bottom
2. User types file path (supports `~` expansion to `$HOME`)
3. `Enter` → validate path:
   - If file exists: show "File already exists" status message, do not write
   - If parent dir missing: show "Parent directory does not exist" status message
   - Otherwise: write via atomic `tempfile::NamedTempFile` pattern (matching `save_edit_to`)
4. Status message: "Exported N snippets to /path/to/file" or error
5. `Esc` cancels path input, returns to compose browsing

### Screen Entry Flow

1. User presses `3` (Normal mode) → `screen = Screen::Compose`
2. If `App.library` is `None`: load library from `library_path()` (same as LibraryBrowse)
3. If `compose_state` is `None`: initialize `ComposeState` with empty selection
4. If `compose_state` is `Some` (returning from tab switch): reuse existing state
5. Empty library: show message "Library is empty. Save snippets with v then s on the Files screen."
6. On library mutation (delete/rename/add snippet): invalidate `compose_state` (set to `None`), forcing re-initialization on next entry

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/compose.rs` | **NEW** — `ComposeState`, `draw_compose_screen()`, `handle_compose_key()`, `handle_export_path_key()` |
| `src/compose.rs` | **NEW** — pure function: `compose_snippets()` |
| `src/tui/app.rs` | Add `Screen::Compose`, `Mode::ExportPath`; add `compose_state: Option<ComposeState>` to `App`; update `draw_tab_bar()` for third tab; update `help_line()` dispatch |
| `src/lib.rs` | Add `pub mod compose;` re-export |

### Edge Cases

- **Empty library:** Show help message, `w` shows "Nothing to export" status
- **No snippets selected:** `w` shows "No snippets selected" status message
- **Library file missing:** Load returns empty library (existing behavior in `load_library`)
- **Library file corrupt:** Show error status, compose screen renders empty
- **File already exists:** Refuse to overwrite, show status message
- **Parent directory missing:** Show error status message
- **Very narrow terminal:** Pane widths collapse gracefully (ratatui percentage layout handles this)
- **Library is `None` when pressing `3`:** Load library first, show error in status if load fails

## Acceptance Criteria

- [ ] Key `3` opens the Compose screen with tab bar showing `[1 Files] [2 Settings] [3 Compose]`
- [ ] Library snippets displayed with `[ ]`/`[x]` checkboxes
- [ ] `j`/`k` navigates the snippet list; cursor is visually highlighted
- [ ] `Space` toggles snippet selection
- [ ] `p` shows full-screen composed preview; `Esc` returns to list
- [ ] `w` enters path input; file is written atomically on `Enter`
- [ ] Path input supports `~` expansion
- [ ] `Esc` returns to Files screen; `q` exits the app
- [ ] Empty library shows a helpful empty-state message
- [ ] Export with no selection shows "No snippets selected" status
- [ ] Help bar shows context-appropriate keybindings for Compose
- [ ] All code passes `cargo clippy --all-targets -- -D warnings`
- [ ] All new functionality covered by unit and integration tests (TDD)

## Dependencies & Risks

| Risk | Mitigation |
|------|------------|
| Large library performance (100+ snippets) | Use scrollable list with viewport pattern (same as SettingsState); compose function is O(n) |
| Compose state lost on app crash | Acceptable for MVP; future: persist compose sessions to disk |
| Stale indices after library mutation | Invalidate `compose_state` on any library change (delete, rename, add) |

## Review Feedback Applied

This plan was revised based on parallel reviews (DHH, Kieran, Simplicity). Key changes:
- Removed reorder (`J`/`K`), clipboard (`arboard`), select all/deselect all — deferred as follow-ups
- Switched from dual-pane to single-pane with preview toggle
- Removed library clone from `ComposeState` — borrow from `App.library` instead
- Renamed `PathInput` → `ExportPath` (intent over mechanism)
- Hardcoded separator instead of parameterizing
- Reduced acceptance criteria from 19 to 13
- Added prerequisite: #19 (app.rs split) must land first
- Added library-None guard and library mutation invalidation

## References

- Brainstorm: `docs/brainstorms/2026-02-28-snippet-library-brainstorm.md`
- Phase 1 implementation: CHANGELOG v0.1.0
- Issue: #1
- Prerequisite: #19 (Split app.rs into per-screen modules)
- Related: #18 (library browse from file list pane)
- Existing patterns: `save_edit_to()` (atomic write), `enter_library_browse_from()` (lazy library load), `SettingsState` (scrollable list with viewport)
