use std::cell::Cell;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::crossterm::event;
use ratatui::crossterm::event::Event;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use tui_textarea::TextArea;
use tui_tree_widget::TreeItem;
use tui_tree_widget::TreeState;

use crate::library::SnippetLibrary;
use crate::model::SourceRoot;
use crate::settings::SettingsCollection;
use crate::settings::SettingsLineMap;
use crate::tui::theme::Theme;

pub type TreeId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Files,
    Settings,
    Compose,
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    FileList,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    VisualSelect,
    TitleInput,
    RenameInput,
    Edit,
    ExportPath,
}

#[derive(Debug)]
pub struct ContentState {
    pub text: Option<String>,
    pub scroll: u16,
    pub cursor: usize,
    pub visual_anchor: Option<usize>,
    /// Captured during draw() — number of visible content lines inside the
    /// border. The event loop always draws before handling input, so this is
    /// populated before any key handler runs.
    pub viewport_height: u16,
}

impl ContentState {
    pub(crate) fn new() -> Self {
        Self {
            text: None,
            scroll: 0,
            cursor: 0,
            visual_anchor: None,
            viewport_height: 0,
        }
    }

    pub fn line_count(&self) -> usize {
        self.text.as_ref().map_or(0, |t| t.lines().count())
    }

    fn max_cursor(&self) -> usize {
        self.line_count().saturating_sub(1)
    }

    pub fn cursor_down(&mut self) {
        if self.cursor < self.max_cursor() {
            self.cursor += 1;
            self.ensure_cursor_visible();
        }
    }

    pub fn cursor_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
        self.ensure_cursor_visible();
    }

    pub fn cursor_page_down(&mut self) {
        let page = (self.viewport_height as usize).max(1);
        self.cursor = (self.cursor + page).min(self.max_cursor());
        self.ensure_cursor_visible();
    }

    pub fn cursor_page_up(&mut self) {
        let page = (self.viewport_height as usize).max(1);
        self.cursor = self.cursor.saturating_sub(page);
        self.ensure_cursor_visible();
    }

    fn ensure_cursor_visible(&mut self) {
        let scroll = self.scroll as usize;
        let vh = self.viewport_height as usize;
        if self.cursor < scroll {
            self.scroll = self.cursor as u16;
        } else if vh > 0 && self.cursor >= scroll + vh {
            self.scroll = (self.cursor - vh + 1) as u16;
        }
    }

    fn load_text(&mut self, raw: String) {
        // Ratatui does not expand tab characters — it treats '\t' as a single-width
        // glyph while the terminal may jump to the next tab stop, causing width
        // mismatches and leftover characters when redrawing. Replace with spaces.
        let text = raw.replace('\t', "    ");
        self.text = Some(text);
        self.scroll = 0;
        self.cursor = 0;
        self.visual_anchor = None;
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.visual_anchor?;
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        let text = self.text.as_ref()?;
        let lines: Vec<&str> = text.lines().collect();
        if start >= lines.len() {
            return None;
        }
        let end = end.min(lines.len().saturating_sub(1));
        Some(lines[start..=end].join("\n"))
    }
}

#[derive(Debug, Default)]
pub struct SettingsState {
    pub lines: Vec<String>,
    pub line_map: SettingsLineMap,
    pub scroll: u16,
    pub cursor: usize,
    pub viewport_height: u16,
    /// When true, displays the effective merged settings instead of per-file view.
    pub merged_view: bool,
    /// Indices of section header lines that are currently collapsed.
    pub collapsed: HashSet<usize>,
}

impl SettingsState {
    pub fn cursor_down(&mut self) {
        // Find the next visible line after the current cursor.
        let next = ((self.cursor + 1)..self.lines.len()).find(|&i| self.is_line_visible(i));
        if let Some(pos) = next {
            self.cursor = pos;
            self.ensure_cursor_visible();
        }
    }

    pub fn cursor_up(&mut self) {
        // Find the previous visible line before the current cursor.
        let prev = (0..self.cursor).rev().find(|&i| self.is_line_visible(i));
        if let Some(pos) = prev {
            self.cursor = pos;
            self.ensure_cursor_visible();
        }
    }

    pub fn cursor_page_down(&mut self) {
        let page = (self.viewport_height as usize).max(1);
        for _ in 0..page {
            let next = ((self.cursor + 1)..self.lines.len()).find(|&i| self.is_line_visible(i));
            match next {
                Some(pos) => self.cursor = pos,
                None => break,
            }
        }
        self.ensure_cursor_visible();
    }

    pub fn cursor_page_up(&mut self) {
        let page = (self.viewport_height as usize).max(1);
        for _ in 0..page {
            let prev = (0..self.cursor).rev().find(|&i| self.is_line_visible(i));
            match prev {
                Some(pos) => self.cursor = pos,
                None => break,
            }
        }
        self.ensure_cursor_visible();
    }

    pub(crate) fn ensure_cursor_visible(&mut self) {
        // Count visible lines before the cursor to determine effective scroll position.
        let visible_before: usize = (0..=self.cursor)
            .filter(|&i| self.is_line_visible(i))
            .count();
        let visible_pos = visible_before.saturating_sub(1);
        let scroll = self.scroll as usize;
        let vh = self.viewport_height as usize;
        if visible_pos < scroll {
            self.scroll = visible_pos as u16;
        } else if vh > 0 && visible_pos >= scroll + vh {
            self.scroll = (visible_pos - vh + 1) as u16;
        }
    }

    /// Returns the indentation depth of a line. `▾`/`▸` headers are depth 0.
    fn indent_depth(line: &str) -> usize {
        if line.starts_with('▾') || line.starts_with('▸') {
            return 0;
        }
        line.len() - line.trim_start().len()
    }

    /// Returns true if the line at the given index has children (the next
    /// non-blank line has deeper indentation).
    pub fn is_foldable(&self, line_idx: usize) -> bool {
        let Some(line) = self.lines.get(line_idx) else {
            return false;
        };
        if line.trim().is_empty() {
            return false;
        }
        let depth = Self::indent_depth(line);
        // Find the next non-blank line.
        self.lines[(line_idx + 1)..]
            .iter()
            .find(|l| !l.trim().is_empty())
            .is_some_and(|next| Self::indent_depth(next) > depth)
    }

    /// Finds the nearest parent line (lesser indentation) above `line_idx`.
    /// Returns `None` if the line is at top level or has no parent.
    pub fn parent_for(&self, line_idx: usize) -> Option<usize> {
        let line = self.lines.get(line_idx)?;
        let depth = Self::indent_depth(line);
        if depth == 0 {
            return None;
        }
        (0..line_idx)
            .rev()
            .find(|&i| Self::indent_depth(&self.lines[i]) < depth)
    }

    /// Returns true if the line at the given index should be displayed.
    /// A line is hidden if any of its ancestors is collapsed.
    pub fn is_line_visible(&self, line_idx: usize) -> bool {
        let Some(line) = self.lines.get(line_idx) else {
            return false;
        };
        if line.trim().is_empty() {
            // Blank separators: visible unless the next non-blank line is
            // hidden (i.e. the section below is folded at depth 0).
            return !self.is_blank_line_hidden(line_idx);
        }
        let depth = Self::indent_depth(line);
        if depth == 0 {
            return true;
        }
        // Walk up through ancestors: if any is collapsed, this line is hidden.
        let mut check = line_idx;
        loop {
            match self.parent_for(check) {
                Some(parent) => {
                    if self.collapsed.contains(&parent) {
                        return false;
                    }
                    check = parent;
                }
                None => return true,
            }
        }
    }

    /// Checks whether a blank separator line should be hidden.
    fn is_blank_line_hidden(&self, line_idx: usize) -> bool {
        // A blank line is a separator between sections. It is hidden if the
        // section *above* it is collapsed at the top level.
        (0..line_idx)
            .rev()
            .find(|&i| !self.lines[i].trim().is_empty())
            .is_some_and(|above| {
                Self::indent_depth(&self.lines[above]) > 0
                    && self
                        .parent_for(above)
                        .is_some_and(|p| self.collapsed.contains(&p))
            })
    }

    /// Toggles the collapsed state of the line at `line_idx`.
    /// Only works on foldable lines. Updates `▾`/`▸` indicator in the line.
    pub fn toggle_fold(&mut self, line_idx: usize) {
        if !self.is_foldable(line_idx) {
            return;
        }
        if self.collapsed.contains(&line_idx) {
            self.collapsed.remove(&line_idx);
            if let Some(line) = self.lines.get_mut(line_idx)
                && let Some(pos) = line.find('▸')
            {
                line.replace_range(pos..pos + 3, "▾");
            }
        } else {
            self.collapsed.insert(line_idx);
            if let Some(line) = self.lines.get_mut(line_idx)
                && let Some(pos) = line.find('▾')
            {
                line.replace_range(pos..pos + 3, "▸");
            }
        }
    }

    /// Returns the visible line count (excluding hidden lines in collapsed
    /// sections).
    pub(crate) fn visible_line_count(&self) -> usize {
        (0..self.lines.len())
            .filter(|&i| self.is_line_visible(i))
            .count()
    }
}

/// State for the text editor when in `Mode::Edit`.
pub struct EditState {
    pub textarea: TextArea<'static>,
    pub file_path: PathBuf,
    pub original_text: String,
    pub had_trailing_newline: bool,
    pub discard_confirmed: bool,
    /// Cached dirty flag. `None` means the cache is stale and must be recomputed.
    /// Using `Cell` allows `is_dirty()` to keep `&self` (needed for Debug and draw).
    pub(crate) dirty_cache: Cell<Option<bool>>,
}

impl std::fmt::Debug for EditState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditState")
            .field("file_path", &self.file_path)
            .field("is_dirty", &self.is_dirty())
            .field("discard_confirmed", &self.discard_confirmed)
            .finish()
    }
}

impl EditState {
    /// Returns true if the textarea content differs from the original text.
    /// Uses an internal cache to avoid re-joining lines on every render frame.
    pub fn is_dirty(&self) -> bool {
        if let Some(cached) = self.dirty_cache.get() {
            return cached;
        }
        let dirty = self.textarea.lines().join("\n") != self.original_text;
        self.dirty_cache.set(Some(dirty));
        dirty
    }

    /// Invalidate the dirty cache. Must be called after any textarea mutation.
    pub fn invalidate_dirty_cache(&self) {
        self.dirty_cache.set(None);
    }
}

#[derive(Debug)]
pub struct App {
    pub exit: bool,
    pub screen: Screen,
    pub mode: Mode,
    pub(crate) tree_state: TreeState<TreeId>,
    pub(crate) tree_items: Vec<TreeItem<'static, TreeId>>,
    pub(crate) active_pane: Pane,
    pub content: ContentState,
    pub title_input: String,
    pub title_cursor: usize,
    pub status_message: Option<String>,
    pub library: Option<SnippetLibrary>,
    pub library_selected: usize,
    pub settings_state: SettingsState,
    pub settings_collection: Option<SettingsCollection>,
    pub edit_state: Option<EditState>,
    pub compose_state: Option<super::compose::ComposeState>,
    /// When editing a library snippet, tracks the index being edited.
    pub editing_snippet_index: Option<usize>,
    pub theme: Theme,
}

impl App {
    pub fn new(roots: Vec<SourceRoot>, config: &crate::config::Config) -> Self {
        let tree_items = build_tree_items(&roots);
        let mut tree_state = TreeState::default();

        // Open all root nodes by default
        for root in &roots {
            tree_state.open(vec![root.path.display().to_string()]);
        }

        // Select the first file under the first root so the app opens with
        // content visible (typically the global CLAUDE.md).
        if let Some(first_root) = roots.first() {
            if let Some(first_file) = first_root.files.first() {
                tree_state.select(vec![
                    first_root.path.display().to_string(),
                    first_file.display().to_string(),
                ]);
            } else {
                tree_state.select_first();
            }
        } else {
            tree_state.select_first();
        }

        let mut app = Self {
            exit: false,
            screen: Screen::Files,
            mode: Mode::Normal,
            tree_state,
            tree_items,
            active_pane: Pane::FileList,
            content: ContentState::new(),
            title_input: String::new(),
            title_cursor: 0,
            status_message: None,
            library: None,
            library_selected: 0,
            settings_state: SettingsState::default(),
            settings_collection: None,
            edit_state: None,
            compose_state: None,
            editing_snippet_index: None,
            theme: match config.theme.as_deref() {
                Some("light") => Theme::light(),
                _ => Theme::dark(),
            },
        };

        app.load_selected_content();
        app
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    pub(crate) fn help_line(&self) -> Line<'static> {
        let key_style = self.theme.help_key;
        let desc_style = self.theme.help_desc;
        let sep = Span::styled("  ", desc_style);

        let pairs: Vec<(&str, &str)> = match self.screen {
            Screen::Compose if self.mode == Mode::ExportPath => {
                vec![("Enter", "Export"), ("Esc", "Cancel")]
            }
            Screen::Compose => {
                vec![
                    ("1", "Files"),
                    ("2", "Settings"),
                    ("3", "Compose"),
                    ("Space", "Toggle"),
                    ("Tab", "Preview"),
                    ("w", "Export"),
                    ("j/k", "Navigate"),
                    ("q", "Quit"),
                ]
            }
            Screen::Settings if self.mode == Mode::Edit => {
                vec![("Ctrl+S", "Save"), ("Esc", "Cancel")]
            }
            Screen::Settings if self.settings_state.merged_view => {
                vec![
                    ("1", "Files"),
                    ("2", "Settings"),
                    ("m", "Per-file"),
                    ("j/k", "Scroll"),
                    ("h/l", "Fold"),
                    ("T", "Theme"),
                    ("q", "Quit"),
                ]
            }
            Screen::Settings => {
                vec![
                    ("1", "Files"),
                    ("2", "Settings"),
                    ("e", "Edit"),
                    ("m", "Merge"),
                    ("j/k", "Scroll"),
                    ("h/l", "Fold"),
                    ("T", "Theme"),
                    ("q", "Quit"),
                ]
            }
            Screen::Files => match self.mode {
                Mode::Normal if self.active_pane == Pane::Content => {
                    vec![
                        ("1", "Files"),
                        ("2", "Settings"),
                        ("q", "Quit"),
                        ("Tab", "Files"),
                        ("j/k", "Scroll"),
                        ("e", "Edit"),
                        ("v", "Select"),
                        ("T", "Theme"),
                    ]
                }
                Mode::Normal => {
                    vec![
                        ("1", "Files"),
                        ("2", "Settings"),
                        ("q", "Quit"),
                        ("Tab", "Content"),
                        ("j/k", "Navigate"),
                        ("T", "Theme"),
                    ]
                }
                Mode::VisualSelect => {
                    vec![("j/k", "Extend"), ("s", "Save"), ("Esc", "Cancel")]
                }
                Mode::TitleInput => {
                    vec![("Enter", "Save"), ("Esc", "Cancel")]
                }
                Mode::Edit => {
                    vec![("Ctrl+S", "Save"), ("Esc", "Cancel")]
                }
                Mode::RenameInput | Mode::ExportPath => {
                    vec![("Enter", "Export"), ("Esc", "Cancel")]
                }
            },
            Screen::Library if self.mode == Mode::RenameInput => {
                vec![("Enter", "Save"), ("Esc", "Cancel")]
            }
            Screen::Library => {
                vec![
                    ("1", "Files"),
                    ("2", "Settings"),
                    ("3", "Compose"),
                    ("4", "Library"),
                    ("j/k", "Navigate"),
                    ("e", "Edit"),
                    ("r", "Rename"),
                    ("d", "Delete"),
                    ("q", "Quit"),
                ]
            }
        };

        let mut spans: Vec<Span> = Vec::new();
        for (i, (key, desc)) in pairs.iter().enumerate() {
            if i > 0 {
                spans.push(sep.clone());
            }
            spans.push(Span::styled(format!(" {key} "), key_style));
            spans.push(Span::styled(format!(" {desc}"), desc_style));
        }
        Line::from(spans)
    }

    pub(crate) fn draw(&mut self, frame: &mut Frame) {
        // Vertical layout: tab_bar + main area + optional input/status bar + help bar
        let has_input_or_status = self.mode == Mode::TitleInput
            || self.mode == Mode::RenameInput
            || self.mode == Mode::ExportPath
            || self.status_message.is_some();

        let mut constraints = vec![Constraint::Length(1), Constraint::Min(3)];
        if has_input_or_status {
            constraints.push(Constraint::Length(3));
        }
        constraints.push(Constraint::Length(1));

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(frame.area());

        let tab_area = vertical[0];
        let main_area = vertical[1];

        // Tab bar
        self.draw_tab_bar(frame, tab_area);

        // Main content area — route by screen
        match self.screen {
            Screen::Files => self.draw_files_screen(frame, main_area),
            Screen::Settings => self.draw_settings_screen(frame, main_area),
            Screen::Compose => self.draw_compose_screen(frame, main_area),
            Screen::Library => self.draw_library_screen(frame, main_area),
        }

        // Input/status bar (when active, Files screen only)
        if has_input_or_status {
            let bar_area = vertical[2];
            if self.mode == Mode::TitleInput
                || self.mode == Mode::RenameInput
                || self.mode == Mode::ExportPath
            {
                let bar_title = match self.mode {
                    Mode::RenameInput => "Rename snippet",
                    Mode::ExportPath => "Export path",
                    _ => "Snippet title",
                };
                let input_widget = Paragraph::new(self.title_input.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.input_border)
                        .title(bar_title),
                );
                frame.render_widget(input_widget, bar_area);
                let cursor_x = bar_area.x + 1 + self.title_cursor as u16;
                let cursor_y = bar_area.y + 1;
                frame.set_cursor_position((cursor_x, cursor_y));
            } else if let Some(msg) = &self.status_message {
                let status_widget = Paragraph::new(msg.as_str())
                    .block(Block::default().borders(Borders::ALL).title("Status"));
                frame.render_widget(status_widget, bar_area);
            }
        }

        // Help bar (always visible, last slot)
        let help_area = vertical[vertical.len() - 1];
        let help = Paragraph::new(self.help_line());
        frame.render_widget(help, help_area);
    }

    fn draw_tab_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let active_style = self.theme.active_tab;
        let inactive_style = self.theme.inactive_tab;

        let style_for = |s: Screen| {
            if self.screen == s {
                active_style
            } else {
                inactive_style
            }
        };

        let tab_line = Line::from(vec![
            Span::styled(" [1 Files] ", style_for(Screen::Files)),
            Span::styled(" [2 Settings] ", style_for(Screen::Settings)),
            Span::styled(" [3 Compose] ", style_for(Screen::Compose)),
            Span::styled(" [4 Library] ", style_for(Screen::Library)),
        ]);
        frame.render_widget(Paragraph::new(tab_line), area);
    }

    pub(crate) fn load_selected_content(&mut self) {
        let selected = self.tree_state.selected();
        if selected.len() < 2 {
            self.content.text = None;
            self.content.scroll = 0;
            self.content.cursor = 0;
            self.content.visual_anchor = None;
            return;
        }

        let file_path = selected.last().cloned();
        if let Some(path_str) = file_path {
            self.load_file_content(&PathBuf::from(path_str));
        }
    }

    pub(crate) fn load_file_content(&mut self, path: &Path) {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => format!("Error reading {}: {err}", path.display()),
        };
        self.content.load_text(text);
    }

    pub(crate) fn reset_to_normal(&mut self) {
        self.mode = Mode::Normal;
        self.content.visual_anchor = None;
        self.title_input.clear();
        self.title_cursor = 0;
    }

    pub(crate) fn current_source_path(&self) -> String {
        self.tree_state
            .selected()
            .last()
            .cloned()
            .unwrap_or_default()
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if let Event::Key(key_event) = event::read()? {
            self.handle_key_event(key_event);
        }
        Ok(())
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        // Clear transient status on any keypress
        self.status_message = None;

        // Ctrl-C always exits regardless of mode
        if key_event.code == KeyCode::Char('c')
            && key_event.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.exit = true;
            return;
        }

        // Screen switching and theme toggle only in Normal mode
        if self.mode == Mode::Normal {
            match key_event.code {
                KeyCode::Char('1') => {
                    self.screen = Screen::Files;
                    return;
                }
                KeyCode::Char('2') => {
                    self.switch_to_settings();
                    return;
                }
                KeyCode::Char('3') => {
                    self.enter_compose_screen();
                    return;
                }
                KeyCode::Char('4') => {
                    self.enter_library_screen();
                    return;
                }
                KeyCode::Char('T') => {
                    self.theme = self.theme.toggle();
                    return;
                }
                _ => {}
            }
        }

        // Edit mode handles its own keys regardless of screen
        if self.mode == Mode::Edit {
            self.handle_edit_key(key_event);
            return;
        }

        match self.screen {
            Screen::Files => match self.mode {
                Mode::Normal => self.handle_normal_key(key_event),
                Mode::VisualSelect => self.handle_visual_select_key(key_event),
                Mode::TitleInput => self.handle_title_input_key(key_event),
                Mode::Edit => {}                           // handled above
                Mode::RenameInput | Mode::ExportPath => {} // not used on Files screen
            },
            Screen::Settings => self.handle_settings_key(key_event),
            Screen::Compose => match self.mode {
                Mode::Normal => self.handle_compose_key(key_event),
                Mode::ExportPath => self.handle_export_path_key(key_event),
                _ => {}
            },
            Screen::Library => match self.mode {
                Mode::Normal => self.handle_library_key(key_event),
                Mode::RenameInput => self.handle_library_rename_key(key_event),
                _ => {}
            },
        }
    }
}

pub fn build_tree_items(roots: &[SourceRoot]) -> Vec<TreeItem<'static, TreeId>> {
    roots
        .iter()
        .filter_map(|root| {
            let root_id = root.path.display().to_string();
            let children: Vec<TreeItem<'static, TreeId>> = root
                .files
                .iter()
                .map(|file| {
                    let file_id = file.display().to_string();
                    let label = file
                        .strip_prefix(&root.path)
                        .unwrap_or(file)
                        .display()
                        .to_string();
                    TreeItem::new_leaf(file_id, label)
                })
                .collect();
            TreeItem::new(root_id, root.path.display().to_string(), children).ok()
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use std::path::PathBuf;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyCode;
    use ratatui::crossterm::event::KeyEvent;
    use ratatui::crossterm::event::KeyEventKind;
    use ratatui::crossterm::event::KeyEventState;
    use ratatui::crossterm::event::KeyModifiers;

    use crate::model::SourceRoot;

    use super::App;

    pub fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    pub fn sample_roots() -> Vec<SourceRoot> {
        vec![
            SourceRoot {
                path: PathBuf::from("/a"),
                files: vec![PathBuf::from("/a/CLAUDE.md")],
            },
            SourceRoot {
                path: PathBuf::from("/b"),
                files: vec![
                    PathBuf::from("/b/CLAUDE.md"),
                    PathBuf::from("/b/sub/CLAUDE.md"),
                ],
            },
        ]
    }

    /// Renders the app once so `TreeState` populates `last_identifiers`,
    /// enabling `key_down()`/`key_up()` navigation in tests.
    pub fn render_once(app: &mut App) {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;
    use crate::config::Config;
    use ratatui::crossterm::event::KeyEventKind;
    use ratatui::crossterm::event::KeyEventState;
    use tempfile::TempDir;

    #[test]
    fn q_key_sets_exit() {
        let mut app = App::new(vec![], &Config::default());
        app.handle_key_event(key_event(KeyCode::Char('q')));
        assert!(app.exit);
    }

    #[test]
    fn other_keys_do_not_exit() {
        let mut app = App::new(vec![], &Config::default());
        app.handle_key_event(key_event(KeyCode::Char('a')));
        assert!(!app.exit);
    }

    #[test]
    fn build_tree_items_creates_correct_hierarchy() {
        let roots = sample_roots();
        let items = build_tree_items(&roots);

        assert_eq!(items.len(), 2, "Should have two root nodes");
        assert_eq!(items[0].children().len(), 1, "First root has one file");
        assert_eq!(items[1].children().len(), 2, "Second root has two files");
    }

    #[test]
    fn first_file_is_selected_and_loaded_on_startup() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Test content").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file.clone()],
        }];
        let app = App::new(roots, &Config::default());

        // The first file should be auto-selected and its content loaded
        assert_eq!(app.content.text.as_deref(), Some("Test content"));
        assert_eq!(
            app.tree_state.selected(),
            vec![tmp.path().display().to_string(), file.display().to_string()]
        );
    }

    // --- Mode transition tests ---

    #[test]
    fn app_starts_in_normal_mode() {
        let app = App::new(vec![], &Config::default());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn q_does_not_exit_in_visual_select_mode() {
        let mut app = App::new(vec![], &Config::default());
        app.mode = Mode::VisualSelect;
        app.handle_key_event(key_event(KeyCode::Char('q')));
        assert!(!app.exit);
    }

    #[test]
    fn q_does_not_exit_in_title_input_mode() {
        let mut app = App::new(vec![], &Config::default());
        app.mode = Mode::TitleInput;
        app.handle_key_event(key_event(KeyCode::Char('q')));
        assert!(!app.exit);
    }

    #[test]
    fn ctrl_c_exits_in_any_mode() {
        for mode in [
            Mode::Normal,
            Mode::VisualSelect,
            Mode::TitleInput,
            Mode::RenameInput,
            Mode::Edit,
        ] {
            let mut app = App::new(vec![], &Config::default());
            app.mode = mode;
            app.handle_key_event(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::empty(),
            });
            assert!(app.exit, "Ctrl-C should exit in {mode:?}");
        }
    }

    #[test]
    fn status_message_cleared_on_keypress() {
        let mut app = App::new(vec![], &Config::default());
        app.status_message = Some("Test message".to_string());
        app.handle_key_event(key_event(KeyCode::Char('a')));
        assert!(app.status_message.is_none());
    }

    #[test]
    fn help_line_shows_edit_key_in_content_pane() {
        let mut app = App::new(vec![], &Config::default());
        app.active_pane = Pane::Content;
        app.mode = Mode::Normal;
        let help = app.help_line();
        let help_text: String = help.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(
            help_text.contains("Edit"),
            "Help line should show Edit in content pane: {help_text}"
        );
    }

    #[test]
    fn help_line_shows_save_cancel_in_edit_mode() {
        let mut app = App::new(vec![], &Config::default());
        app.mode = Mode::Edit;
        let help = app.help_line();
        let help_text: String = help.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(
            help_text.contains("Save") && help_text.contains("Cancel"),
            "Help line should show Save and Cancel in edit mode: {help_text}"
        );
    }

    #[test]
    fn shift_t_toggles_theme_in_normal_mode() {
        let mut app = App::new(vec![], &Config::default());
        assert!(app.theme.is_dark);

        let shift_t = KeyEvent {
            code: KeyCode::Char('T'),
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key_event(shift_t);
        assert!(!app.theme.is_dark, "Theme should toggle to light");

        app.handle_key_event(shift_t);
        assert!(app.theme.is_dark, "Theme should toggle back to dark");
    }

    #[test]
    fn shift_t_ignored_in_edit_mode() {
        let mut app = App::new(vec![], &Config::default());
        // Force into edit mode by setting mode directly
        app.mode = Mode::Edit;

        let shift_t = KeyEvent {
            code: KeyCode::Char('T'),
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key_event(shift_t);
        assert!(app.theme.is_dark, "Theme should not toggle in edit mode");
    }

    #[test]
    fn shift_t_ignored_in_title_input_mode() {
        let mut app = App::new(vec![], &Config::default());
        app.mode = Mode::TitleInput;

        let shift_t = KeyEvent {
            code: KeyCode::Char('T'),
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key_event(shift_t);
        assert!(
            app.theme.is_dark,
            "Theme should not toggle in title input mode"
        );
    }

    #[test]
    fn shift_t_toggles_on_settings_screen() {
        let mut app = App::new(vec![], &Config::default());
        app.screen = Screen::Settings;

        let shift_t = KeyEvent {
            code: KeyCode::Char('T'),
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key_event(shift_t);
        assert!(!app.theme.is_dark, "Theme should toggle on settings screen");
    }

    #[test]
    fn config_theme_light_starts_in_light_mode() {
        let config = Config {
            theme: Some("light".to_string()),
            ..Config::default()
        };
        let app = App::new(vec![], &config);
        assert!(!app.theme.is_dark);
    }

    #[test]
    fn config_theme_dark_starts_in_dark_mode() {
        let config = Config {
            theme: Some("dark".to_string()),
            ..Config::default()
        };
        let app = App::new(vec![], &config);
        assert!(app.theme.is_dark);
    }

    #[test]
    fn config_theme_none_defaults_to_dark() {
        let config = Config::default();
        let app = App::new(vec![], &config);
        assert!(app.theme.is_dark);
    }
}
