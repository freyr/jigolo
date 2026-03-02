---
date: 2026-02-28
topic: settings-viewer
---

# Settings Viewer — Read-Only Claude Code Configuration Browser

## What We're Building

A new top-level screen in the TUI for viewing Claude Code settings files. Users switch between screens using number keys: `1` for the CLAUDE.md browser (current default), `2` for the Settings viewer. The settings screen uses the full width (no left tree pane) and displays a structured, labeled view of all settings — permissions, MCP servers, hooks, model, env vars, plugins.

Phase 1 is read-only. Phase 2 (future) adds inline editing.

## Why This Approach

- **Number-key switching** (`1`/`2`) is fast, discoverable, and scales to future screens (`3` for compose, etc.)
- **Full-width layout** gives settings room to breathe and leaves space for a future right-side detail/edit panel
- **Structured view** (not raw JSON) makes settings scannable without parsing JSON mentally
- **Read-only first** ships value quickly — seeing what's configured is the most common need

## Key Decisions

- **Screen switching via `1`/`2`**: Works in Normal mode only. `1` = CLAUDE.md browser (default), `2` = Settings. The help bar and top of screen should indicate the active screen.
- **Settings files discovered**: `~/.claude/settings.json` (global), `.claude/settings.json` (project shared), `.claude/settings.local.json` (project local). Missing files are silently skipped.
- **Structured display**: Parse JSON into labeled sections — Model, Default Mode, Permissions (allow/ask/deny), MCP Servers, Hooks, Plugins, Env Vars. Each section is collapsible (future) or just scrollable.
- **Full width**: Settings screen replaces both panes. Right side is reserved for future detail/edit panel but is empty in Phase 1.
- **j/k scrolls**: Cursor navigation through the settings content, same as content pane.
- **Esc/q**: Quit the app (same as Normal mode — settings is a top-level screen, not a sub-mode).

## Layout

```
┌─[1 Files]─[2 Settings]─────────────────────────────────┐
│                                                         │
│ ▾ Global (~/.claude/settings.json)                      │
│   Model: opus[1m]                                       │
│   Default Mode: acceptEdits                             │
│   Thinking: always enabled                              │
│                                                         │
│   Permissions (allow):                                  │
│     mcp__github__*                                      │
│     Bash(*)                                             │
│     WebSearch                                           │
│   Permissions (ask):                                    │
│     Bash(rm *)                                          │
│     mcp__github__merge_pull_request                     │
│                                                         │
│   MCP Servers:                                          │
│     context7 (http) → https://mcp.context7.com/mcp      │
│     anki (stdio) → node .../index.js                   │
│                                                         │
│   Hooks:                                                │
│     PreToolUse/Bash → gh-auth-switch.sh                 │
│                                                         │
│   Plugins:                                              │
│     compound-engineering (enabled)                      │
│     rust-analyzer-lsp (enabled)                         │
│                                                         │
│ ▾ Project (.claude/settings.local.json)                 │
│   Permissions (allow):                                  │
│     mcp__plugin_compound-engineering_context7__*         │
│                                                         │
├─────────────────────────────────────────────────────────┤
│  1  Files   2  Settings   j/k  Scroll   q  Quit        │
└─────────────────────────────────────────────────────────┘
```

## Open Questions

- Should the tab bar at the top be a persistent UI element on both screens, or just indicated in the help bar?
- When editing comes in Phase 2, should edits target global or project settings? Likely need a scope selector.
- Should we show the "effective merged" settings (global + project), or always show them separately?

## Next Steps

Plan the implementation for Phase 1 (read-only structured settings viewer with `1`/`2` screen switching).
