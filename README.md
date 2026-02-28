# Jigolo

A TUI for browsing and managing [Claude Code](https://claude.com/claude-code) context files (`CLAUDE.md`). Discover files across directory trees, read them in a dual-pane browser, and build a personal snippet library of reusable rules and patterns.

## Installation

### Homebrew

```sh
brew install freyr/tap/jigolo
```

### From source

```sh
cargo install --path .
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/freyr/jigolo/releases).

Shell installer (macOS/Linux):

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/freyr/jigolo/releases/latest/download/jigolo-installer.sh | sh
```

## Usage

```sh
jigolo                    # Browse current directory
jigolo /path1 /path2     # Browse specific directories
jigolo --list /path      # List files and exit (no TUI)
```

The TUI opens with a dual-pane layout: file tree on the left, content on the right.

### Keybindings

| Key | Action |
|-----|--------|
| `j/k` | Navigate / scroll |
| `Tab` | Switch pane |
| `Enter` | Open/select |
| `v` | Start visual line selection |
| `s` | Save selection as snippet |
| `L` | Open snippet library |
| `r` | Rename snippet (in library) |
| `d` | Delete snippet (in library) |
| `Esc` | Cancel / go back |
| `q` | Quit |

### Snippet Library

Select text you want to reuse across projects:

1. Navigate to a CLAUDE.md file and switch to the content pane (`Tab`)
2. Move cursor to the start line (`j/k`)
3. Press `v` to begin visual selection
4. Extend selection with `j/k`
5. Press `s`, type a title, press `Enter`

Snippets are saved to `~/.config/jigolo/library.toml`. Press `L` to browse, rename (`r`), or delete (`d`) saved snippets.

## License

MIT
