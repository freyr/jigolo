---
title: "fix: Allow folder selection and clear content pane"
type: fix
date: 2026-03-02
issue: "#11"
---

# Fix: Allow folder selection and clear content pane (#11)

## Context

The v0.3.0 fix (`2376365`) tried to skip folder nodes during j/k navigation, but introduced three bugs:
1. The first folder in the tree is still selectable (skip only works for subsequent folders)
2. Left arrow navigates to the parent folder — no skip logic, cursor lands on folder node
3. A second Left press folds the folder, a third Left press clears the selection entirely — cursor disappears

The approach of skipping folder nodes is fragile. The simpler fix (per issue comment): **allow folder nodes to be selected, but clear the content pane** when one is selected.

## Changes

### 1. Remove `skip_root_node_down()` and `skip_root_node_up()` (~line 741-752)

Delete both methods entirely. They are only called from the j/k handlers.

### 2. Remove skip calls from j/k handlers (~line 1099-1108)

Change from:
```rust
KeyCode::Down | KeyCode::Char('j') if self.active_pane == Pane::FileList => {
    self.tree_state.key_down();
    self.skip_root_node_down();
    self.load_selected_content();
}
KeyCode::Up | KeyCode::Char('k') if self.active_pane == Pane::FileList => {
    self.tree_state.key_up();
    self.skip_root_node_up();
    self.load_selected_content();
}
```

To:
```rust
KeyCode::Down | KeyCode::Char('j') if self.active_pane == Pane::FileList => {
    self.tree_state.key_down();
    self.load_selected_content();
}
KeyCode::Up | KeyCode::Char('k') if self.active_pane == Pane::FileList => {
    self.tree_state.key_up();
    self.load_selected_content();
}
```

### 3. Update `load_selected_content()` to clear content on folder nodes (~line 754)

Change from:
```rust
fn load_selected_content(&mut self) {
    let selected = self.tree_state.selected();
    if selected.len() < 2 {
        return;  // silent no-op — stale content stays visible
    }
    ...
}
```

To:
```rust
fn load_selected_content(&mut self) {
    let selected = self.tree_state.selected();
    if selected.len() < 2 {
        self.content.text = None;  // clear content when folder is selected
        self.content.scroll = 0;
        self.content.cursor = 0;
        self.content.visual_anchor = None;
        return;
    }
    ...
}
```

### 4. Tests — TDD sequence

**Red (new tests):**
- `jk_can_land_on_folder_node` — press j/k to navigate to a folder node, assert it is selected (`selected().len() == 1`)
- `folder_selection_clears_content_pane` — select a folder node, call `load_selected_content()`, assert `content.text` is `None`
- `left_arrow_to_parent_clears_content` — from a file, press Left, assert cursor is on folder and content is cleared

**Green:** Apply changes from steps 1-3.

**Update existing tests:**
- Remove/update any tests that assumed folders are skipped during j/k navigation

## Files to modify

- `src/tui/app.rs` — skip methods, key handlers, `load_selected_content()`, tests

## Verification

1. `cargo-clippy` with `all_targets` and `warnings_as_errors`
2. `cargo-test` — all tests pass
3. Manual: run `cargo run` with multiple paths, verify:
   - j/k can land on folder nodes
   - Content pane clears when folder is selected
   - Left arrow from first file goes to folder, content clears
   - No invisible cursor state — selection always visible
