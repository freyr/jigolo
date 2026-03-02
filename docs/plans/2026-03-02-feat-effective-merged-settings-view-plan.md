---
title: "feat: Effective merged settings view"
type: feat
date: 2026-03-02
issue: "#9"
brainstorm: docs/brainstorms/2026-02-28-settings-viewer-brainstorm.md
---

# Effective Merged Settings View

## Context

The settings screen shows Claude Code settings files separately (Global, Project, Project Local). Users want to see the effective result of layering all three files together — what Claude Code actually uses at runtime. This is the most common need when checking "is my override working?"

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Toggle state location | `merged_view: bool` on `SettingsState` | Self-contained with display state |
| Toggle key | `m` | Unbound, mnemonic |
| Cursor/scroll on toggle | Reset to 0 | Different line counts make position preservation meaningless |
| Toggle persistence across screen switches | Reset to per-file on `2` press | Simple, consistent with `apply_settings_collection()` |
| Scalar merge (model, defaultMode, thinking) | Last-writer-wins (Local > Project > Global) | Standard layered config |
| permissions.allow/ask/deny | Set union, deduplicated, first-occurrence order | Matches Claude Code runtime |
| mcpServers | Full object replacement per key, later file wins | Simple, matches most config systems |
| hooks | Per-event array concatenation (additive) | All hooks from all files should fire |
| plugins | Set union, deduplicated | Same as permissions |
| env | Last-writer-wins per key | Standard env override |
| Unrecognized keys | Last-writer-wins | Consistent with scalar rule |
| Invalid JSON file | Skip silently in merge | Merge what we can |
| `e` in merged view | Disabled, show status message | No single file to edit |
| Provenance tags | **None in v1** | Per-file view (press `m`) is the provenance escape hatch. Per-line tags are complex and deliver marginal value. |
| Pane title | `"Settings"` / `"Settings — Effective"` | Visual mode indicator |
| Help bar (per-file) | Add `("m", "Merge")` | Short label |
| Help bar (merged) | `("m", "Per-file")`, omit `e` | `e` disabled in merged |
| `e` disabled message | `"Edit not available in merged view — press m to switch."` | Actionable |
| `merge_settings` return type | `serde_json::Value` (not `SettingsFile`) | Avoid sentinel `PathBuf::new()` and type confusion |

## Changes

### Phase 1: Merge logic in `src/settings.rs`

#### 1a. Extract `ORDERED_SETTINGS_KEYS` constant

Move the inline `ordered_keys` array from `format_settings_with_map` to a module-level constant, shared by both formatters.

#### 1b. Add `merge_settings` function

```rust
/// Merges settings files into a single effective JSON value.
///
/// Scalars: last-writer-wins. Arrays (permissions, plugins): set union.
/// Objects (mcpServers, env): merge by key, later wins.
/// Hooks: per-event array concatenation.
pub fn merge_settings(collection: &SettingsCollection) -> serde_json::Value
```

Returns a `serde_json::Value::Object` (or `Value::Null` for empty collections).

Algorithm:
1. Start with empty `serde_json::Map`
2. Iterate `collection.files` in order (Global first, Local last)
3. Skip files where `value.as_object()` returns `None` (invalid JSON)
4. For each key in the file's object:
   - `permissions`: deep merge — union arrays per sub-key (`allow`, `ask`, `deny`), deduplicate
   - `hooks`: deep merge — per-event key, concatenate arrays
   - `plugins`: union arrays, deduplicate
   - Everything else: last-writer-wins (replace)

No new formatting code — the merged `Value` is wrapped in a synthetic `SettingsCollection` and passed to the existing `format_settings_with_map`.

### Phase 2: Toggle in `src/tui/app.rs`

#### 2a. Add `merged_view: bool` to `SettingsState`

Default `false` (via `#[derive(Default)]`).

#### 2b. Refactor `apply_settings_collection` to delegate to `rebuild_settings_display`

```rust
fn apply_settings_collection(&mut self, collection: SettingsCollection) {
    self.settings_collection = Some(collection);
    self.settings_state.merged_view = false;
    self.rebuild_settings_display();
}

fn rebuild_settings_display(&mut self) {
    let Some(collection) = &self.settings_collection else { return; };
    let (lines, line_map) = if self.settings_state.merged_view {
        let merged = merge_settings(collection);
        let synthetic = SettingsCollection {
            files: vec![SettingsFile {
                label: "Effective".to_string(),
                path: PathBuf::new(),
                value: merged,
            }],
        };
        format_settings_with_map(&synthetic)
    } else {
        format_settings_with_map(collection)
    };
    self.settings_state.lines = lines;
    self.settings_state.line_map = line_map;
    self.settings_state.scroll = 0;
    self.settings_state.cursor = 0;
}
```

#### 2c. Add `m` key handler in `handle_settings_key()`

```rust
KeyCode::Char('m') => {
    self.settings_state.merged_view = !self.settings_state.merged_view;
    self.rebuild_settings_display();
}
```

#### 2d. Guard `e` key in merged view

```rust
KeyCode::Char('e') if !self.settings_state.merged_view => {
    self.enter_settings_edit_mode();
}
KeyCode::Char('e') => {
    self.status_message = Some(
        "Edit not available in merged view — press m to switch.".to_string()
    );
}
```

#### 2e. Fix `status_message` visibility on Settings screen

The `has_input_or_status` guard at ~line 404 is gated on `Screen::Files`. Extend it to include `Screen::Settings`:

```rust
let has_input_or_status = (self.screen == Screen::Files || self.screen == Screen::Settings)
    && (...conditions...);
```

Or simplify: remove the `Screen::Files` guard and show status messages on any screen.

#### 2f. Update `draw_settings_screen()` title

```rust
let title = if self.settings_state.merged_view {
    "Settings — Effective"
} else {
    "Settings"
};
```

#### 2g. Update `help_line()` for settings screen

Per-file mode:
```rust
vec![("1", "Files"), ("2", "Settings"), ("e", "Edit"), ("m", "Merge"), ("j/k", "Scroll"), ("q", "Quit")]
```

Merged mode:
```rust
vec![("1", "Files"), ("2", "Settings"), ("m", "Per-file"), ("j/k", "Scroll"), ("q", "Quit")]
```

### Phase 3: Tests (TDD)

#### `src/settings.rs` tests:
- `merge_scalars_last_writer_wins` — two files with `model`, later wins
- `merge_permissions_are_additive` — two files with overlapping `allow` arrays, deduplicated
- `merge_mcp_servers_by_key` — same server name in two files, later wins
- `merge_hooks_concatenated` — same event in two files, arrays joined
- `merge_env_last_writer_wins` — same env key, later wins
- `merge_plugins_deduplicated` — overlapping plugin arrays
- `merge_skips_invalid_json` — one valid + one invalid file, merge uses valid only

#### `src/tui/app.rs` tests:
- `m_key_toggles_merged_view` — flag flips, lines change, single header
- `m_key_resets_cursor` — cursor and scroll reset to 0 on toggle
- `m_key_round_trip` — toggle twice returns identical per-file lines
- `e_disabled_in_merged_view` — mode stays Normal, status message set
- `help_bar_shows_merge_key` — per-file help includes `m`
- `help_bar_in_merged_omits_edit` — merged help has no `e`

## Files to modify

- `src/settings.rs` — extract constant, add `merge_settings()`, tests
- `src/tui/app.rs` — `SettingsState.merged_view`, `rebuild_settings_display()`, refactor `apply_settings_collection()`, `handle_settings_key()`, `draw_settings_screen()`, `help_line()`, fix status message visibility, tests

## Verification

1. `cargo-clippy` with `all_targets` and `warnings_as_errors`
2. `cargo-test` — all tests pass
3. Manual: `cargo run`, press `2` for settings, press `m` to toggle, verify:
   - Merged view shows single "Effective" section
   - `e` shows status message in merged view
   - `m` toggles back to per-file view
   - Help bar and title update correctly
