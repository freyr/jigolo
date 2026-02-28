---
date: 2026-02-28
topic: snippet-library
---

# Snippet Library — Capture and Compose CLAUDE.md Sections

## What We're Building

A personal library of reusable CLAUDE.md snippets that lives in a single structured file. Users browse existing CLAUDE.md files in the TUI, find interesting sections or rules, and save them to the library. Later (phase 2), they pick items from the library and compose new CLAUDE.md files.

## Phasing

**Phase 1 — Capture (this phase):**
- Visual selection in the content pane (vim-style line selection)
- Save selected text as a library entry with title + optional tags
- Library stored as a single structured file (TOML or JSON)
- Storage location: platform data dir (`~/.config/context-manager/` or similar), configurable later
- Browse/view saved library entries within the TUI

**Phase 2 — Compose (future):**
- Separate TUI mode/screen for assembling new CLAUDE.md from library entries
- Pick, reorder, and export selected snippets to a new file

## Snippet Granularity

Users want to save all of:
- Full markdown heading blocks (## heading + body until next heading)
- Arbitrary multi-line selections
- Individual rules/bullets

This means selection must be line-based and flexible, not tied to markdown structure.

## Key Decisions

- **Single file storage**: One structured file, not a directory of files. Simpler to manage, ship, and back up.
- **Capture first**: Build the save/browse workflow before compose. Let the library grow organically.
- **Platform data dir**: Use standard OS location initially, make configurable later for brew/package distribution.
- **Vim-style visual select**: Fits the existing keybinding model (hjkl navigation already in place).

## Open Questions

- File format: TOML vs JSON vs something else? TOML is human-readable, JSON is simpler to serialize.
- Should entries track their source file path for provenance?
- Tag/category UX: free-form tags, or predefined categories, or both?
- How to handle duplicate/overlapping snippets?

## Next Steps

Plan the implementation for Phase 1 (capture + browse library).
