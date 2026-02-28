# Changelog

## 0.1.0

Initial release.

### Features

- **Dual-pane TUI browser** — left pane shows a tree of discovered CLAUDE.md files, right pane displays content with vim-style cursor navigation (j/k, PageUp/PageDown)
- **Recursive discovery** — walks directory trees finding CLAUDE.md files, skips node_modules/.git/target and other noise directories
- **Global CLAUDE.md** — auto-discovers `~/.claude/CLAUDE.md` and prepends it to the file list
- **Snippet capture** — visual line selection (`v`), title input (`s`), saves to `~/.config/context-manager/library.toml`
- **Library browser** — press `L` to browse saved snippets with split list/preview pane
- **Snippet management** — rename (`r`) and delete (`d`) snippets from the library browser
- **Context-sensitive help bar** — bottom bar shows available keybindings for the current mode
- **List mode** — `--list` flag prints discovered files and exits (no TUI)

### Keybindings

| Key | Action |
|-----|--------|
| `j/k` | Navigate / scroll |
| `Tab` | Switch pane |
| `Enter` | Open/select |
| `v` | Start visual selection |
| `s` | Save selection as snippet |
| `L` | Open library browser |
| `r` | Rename snippet (in library) |
| `d` | Delete snippet (in library) |
| `Esc` | Cancel / go back |
| `q` | Quit |
