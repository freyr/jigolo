---
title: "fix: Remove misleading Enter keybinding from file tree"
type: fix
date: 2026-03-02
issue: "#12"
---

# Fix: Remove misleading Enter keybinding from file tree

## Context

The help bar in the file tree pane shows `Enter` as "Open" but:
- On folders, Enter toggles fold/unfold — duplicating left/right arrow keys
- On files, Enter reloads content that j/k already loaded — effectively a no-op
- The label "Open" misleads users into expecting editor behavior

Decision: remove the Enter keybinding entirely from the file tree. Folder toggling is already handled by `h/l` and arrow keys.

## Changes

### 1. `src/tui/app.rs` — Remove Enter handler (~line 1114)

Delete this match arm from `handle_normal_key()`:

```rust
KeyCode::Enter if self.active_pane == Pane::FileList => {
    self.select_tree_item();
}
```

### 2. `src/tui/app.rs` — Remove Enter from help bar (~line 366)

Change the Normal/FileList help bar from:

```rust
vec![("1","Files"), ("2","Settings"), ("q","Quit"), ("Tab","Content"),
     ("j/k","Navigate"), ("Enter","Open")]
```

To:

```rust
vec![("1","Files"), ("2","Settings"), ("q","Quit"), ("Tab","Content"),
     ("j/k","Navigate")]
```

### 3. `src/tui/app.rs` — Update existing tests

- `select_tree_item_on_root_toggles` (~line 1530): Remove or repurpose — Enter no longer calls `select_tree_item()` from the key handler. The `select_tree_item()` method itself can stay if still called internally, but the test should verify that pressing Enter in FileList is now a no-op.
- `select_tree_item_loads_file_content` (~line 1578): Same — update to verify Enter does nothing on file nodes.
- Other tests referencing Enter + FileList: update accordingly.

### 4. TDD sequence

1. **Red:** Write a test `enter_in_file_list_is_noop` — press Enter on a file node, assert `active_pane`, `mode`, and `content.text` are unchanged from the pre-Enter state.
2. **Green:** Remove the `KeyCode::Enter` arm and the help bar entry.
3. **Verify:** Run `cargo test`, `cargo clippy --all-targets -- -D warnings`.

### 5. `select_tree_item()` method

Keep the method — it may be useful later. Removing it is optional cleanup. If it becomes dead code, clippy will flag it.

## Files to modify

- `src/tui/app.rs` — handler, help bar, tests (single file)

## Verification

1. `cargo-clippy` with `all_targets` and `warnings_as_errors`
2. `cargo-test` — all tests pass
3. Manual: run `cargo run`, verify Enter does nothing in file tree, help bar no longer shows "Enter Open"
