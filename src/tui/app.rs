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
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;
use ratatui::widgets::ScrollbarState;
use tui_tree_widget::Tree;
use tui_tree_widget::TreeItem;
use tui_tree_widget::TreeState;

use crate::library::SnippetLibrary;
use crate::model::SourceRoot;

pub type TreeId = String;

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
    LibraryBrowse,
    RenameInput,
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
    fn new() -> Self {
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

#[derive(Debug)]
pub struct App {
    pub exit: bool,
    pub mode: Mode,
    tree_state: TreeState<TreeId>,
    tree_items: Vec<TreeItem<'static, TreeId>>,
    active_pane: Pane,
    pub content: ContentState,
    pub title_input: String,
    pub status_message: Option<String>,
    pub library: Option<SnippetLibrary>,
    pub library_selected: usize,
}

impl App {
    pub fn new(roots: Vec<SourceRoot>) -> Self {
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
            mode: Mode::Normal,
            tree_state,
            tree_items,
            active_pane: Pane::FileList,
            content: ContentState::new(),
            title_input: String::new(),
            status_message: None,
            library: None,
            library_selected: 0,
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

    fn help_line(&self) -> Line<'static> {
        let key_style = Style::default()
            .fg(Color::Black)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(Color::DarkGray);
        let sep = Span::styled("  ", desc_style);

        let pairs: Vec<(&str, &str)> = match self.mode {
            Mode::Normal if self.active_pane == Pane::Content => {
                vec![
                    ("q", "Quit"),
                    ("Tab", "Files"),
                    ("j/k", "Scroll"),
                    ("v", "Select"),
                    ("L", "Library"),
                ]
            }
            Mode::Normal => {
                vec![
                    ("q", "Quit"),
                    ("Tab", "Content"),
                    ("j/k", "Navigate"),
                    ("Enter", "Open"),
                ]
            }
            Mode::VisualSelect => {
                vec![("j/k", "Extend"), ("s", "Save"), ("Esc", "Cancel")]
            }
            Mode::TitleInput => {
                vec![("Enter", "Save"), ("Esc", "Cancel")]
            }
            Mode::LibraryBrowse => {
                vec![
                    ("j/k", "Navigate"),
                    ("r", "Rename"),
                    ("d", "Delete"),
                    ("Esc", "Back"),
                ]
            }
            Mode::RenameInput => {
                vec![("Enter", "Save"), ("Esc", "Cancel")]
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

    fn draw(&mut self, frame: &mut Frame) {
        // Vertical layout: main area + optional input/status bar + help bar
        let has_input_or_status = self.mode == Mode::TitleInput
            || self.mode == Mode::RenameInput
            || self.status_message.is_some();
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if has_input_or_status {
                vec![
                    Constraint::Min(3),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ]
            } else {
                vec![Constraint::Min(3), Constraint::Length(1)]
            })
            .split(frame.area());

        let main_area = vertical[0];

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(main_area);

        let file_border_style = if self.active_pane == Pane::FileList {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let content_border_style = if self.active_pane == Pane::Content {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        if let Ok(tree) = Tree::new(&self.tree_items) {
            let tree = tree
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(file_border_style)
                        .title("CLAUDE.md files"),
                )
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            frame.render_stateful_widget(tree, chunks[0], &mut self.tree_state);
        }

        if self.mode == Mode::LibraryBrowse || self.mode == Mode::RenameInput {
            self.draw_library_pane(frame, chunks[1], content_border_style);
        } else {
            self.draw_content_pane(frame, chunks[1], content_border_style);
        }

        // Input/status bar (when active)
        if has_input_or_status {
            let bar_area = vertical[1];
            if self.mode == Mode::TitleInput || self.mode == Mode::RenameInput {
                let bar_title = if self.mode == Mode::RenameInput {
                    "Rename snippet"
                } else {
                    "Snippet title"
                };
                let input_widget = Paragraph::new(self.title_input.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow))
                        .title(bar_title),
                );
                frame.render_widget(input_widget, bar_area);
                let cursor_x = bar_area.x + 1 + self.title_input.len() as u16;
                let cursor_y = bar_area.y + 1;
                frame.set_cursor_position((cursor_x, cursor_y));
            } else if let Some(msg) = &self.status_message {
                let status_widget = Paragraph::new(msg.as_str())
                    .block(Block::default().borders(Borders::ALL).title("Status"));
                frame.render_widget(status_widget, bar_area);
            }
        }

        // Help bar (always visible)
        let help_area = if has_input_or_status {
            vertical[2]
        } else {
            vertical[1]
        };
        let help = Paragraph::new(self.help_line());
        frame.render_widget(help, help_area);
    }

    fn draw_content_pane(
        &mut self,
        frame: &mut Frame,
        area: ratatui::layout::Rect,
        border_style: Style,
    ) {
        let content_title = match self.mode {
            Mode::VisualSelect | Mode::TitleInput => {
                if let Some((start, end)) = self.content.selection_range() {
                    format!("Content [VISUAL: lines {}-{}]", start + 1, end + 1)
                } else {
                    "Content [VISUAL]".to_string()
                }
            }
            _ => "Content".to_string(),
        };

        // Capture viewport height (content area minus 2 for borders)
        self.content.viewport_height = area.height.saturating_sub(2);

        let display_text = self
            .content
            .text
            .as_deref()
            .unwrap_or("Select a file to view its content.");

        let selection = self.content.selection_range();
        let cursor_line = self.content.cursor;
        let show_cursor = self.active_pane == Pane::Content;
        let cursor_style = Style::default().add_modifier(Modifier::UNDERLINED);
        let highlight_style = Style::default().bg(Color::DarkGray);

        let lines: Vec<Line> = display_text
            .lines()
            .enumerate()
            .map(|(i, line_text)| {
                let mut style = Style::default();
                if let Some((start, end)) = selection
                    && i >= start
                    && i <= end
                {
                    style = highlight_style;
                }
                if show_cursor && i == cursor_line {
                    style = style.add_modifier(Modifier::UNDERLINED);
                    if selection.is_none() {
                        style = cursor_style;
                    }
                }
                Line::from(line_text.to_string()).style(style)
            })
            .collect();

        let content_widget = Paragraph::new(Text::from(lines))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(content_title),
            )
            .scroll((self.content.scroll, 0));
        frame.render_widget(content_widget, area);

        let mut scrollbar_state =
            ScrollbarState::new(self.content.line_count()).position(self.content.scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }

    fn draw_library_pane(
        &self,
        frame: &mut Frame,
        area: ratatui::layout::Rect,
        border_style: Style,
    ) {
        let lib = match &self.library {
            Some(lib) => lib,
            None => return,
        };

        if lib.snippets.is_empty() {
            let empty_msg = Paragraph::new("No snippets saved. Use v to select, s to save.").block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title("Library (empty)"),
            );
            frame.render_widget(empty_msg, area);
            return;
        }

        let lib_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        // Snippet list (top)
        let list_title = format!("Library ({} snippets)", lib.snippets.len());
        let list_lines: Vec<Line> = lib
            .snippets
            .iter()
            .enumerate()
            .map(|(i, snippet)| {
                let style = if i == self.library_selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                Line::from(format!("  {}", snippet.title)).style(style)
            })
            .collect();
        let list_widget = Paragraph::new(Text::from(list_lines)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(list_title),
        );
        frame.render_widget(list_widget, lib_split[0]);

        // Preview (bottom)
        let preview_content = lib
            .snippets
            .get(self.library_selected)
            .map(|s| s.content.as_str())
            .unwrap_or("");
        let preview_title = lib
            .snippets
            .get(self.library_selected)
            .map(|s| format!("Preview: {}", s.title))
            .unwrap_or_else(|| "Preview".to_string());
        let preview_widget = Paragraph::new(preview_content).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(preview_title),
        );
        frame.render_widget(preview_widget, lib_split[1]);
    }

    fn select_tree_item(&mut self) {
        let selected = self.tree_state.selected();
        if selected.is_empty() {
            return;
        }

        // A root node has exactly one identifier segment; a file has two.
        if selected.len() == 1 {
            self.tree_state.toggle_selected();
        }

        self.load_selected_content();
    }

    fn load_selected_content(&mut self) {
        let selected = self.tree_state.selected();
        if selected.len() < 2 {
            return;
        }

        let file_path = selected.last().cloned();
        if let Some(path_str) = file_path {
            self.load_file_content(&PathBuf::from(path_str));
        }
    }

    fn load_file_content(&mut self, path: &Path) {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => format!("Error reading {}: {err}", path.display()),
        };
        self.content.load_text(text);
    }

    fn reset_to_normal(&mut self) {
        self.mode = Mode::Normal;
        self.content.visual_anchor = None;
        self.title_input.clear();
    }

    fn current_source_path(&self) -> String {
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

        match self.mode {
            Mode::Normal => self.handle_normal_key(key_event),
            Mode::VisualSelect => self.handle_visual_select_key(key_event),
            Mode::TitleInput => self.handle_title_input_key(key_event),
            Mode::LibraryBrowse => self.handle_library_browse_key(key_event),
            Mode::RenameInput => self.handle_rename_input_key(key_event),
        }
    }

    fn handle_normal_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit = true,
            KeyCode::Tab => {
                self.active_pane = match self.active_pane {
                    Pane::FileList => Pane::Content,
                    Pane::Content => Pane::FileList,
                };
            }
            KeyCode::Enter if self.active_pane == Pane::FileList => {
                self.select_tree_item();
            }
            KeyCode::Down | KeyCode::Char('j') if self.active_pane == Pane::FileList => {
                self.tree_state.key_down();
                self.load_selected_content();
            }
            KeyCode::Up | KeyCode::Char('k') if self.active_pane == Pane::FileList => {
                self.tree_state.key_up();
                self.load_selected_content();
            }
            KeyCode::Left | KeyCode::Char('h') if self.active_pane == Pane::FileList => {
                self.tree_state.key_left();
                self.load_selected_content();
            }
            KeyCode::Right | KeyCode::Char('l') if self.active_pane == Pane::FileList => {
                self.tree_state.key_right();
                self.load_selected_content();
            }
            KeyCode::Down | KeyCode::Char('j') if self.active_pane == Pane::Content => {
                self.content.cursor_down();
            }
            KeyCode::Up | KeyCode::Char('k') if self.active_pane == Pane::Content => {
                self.content.cursor_up();
            }
            KeyCode::PageDown if self.active_pane == Pane::Content => {
                self.content.cursor_page_down();
            }
            KeyCode::PageUp if self.active_pane == Pane::Content => {
                self.content.cursor_page_up();
            }
            KeyCode::Char('v') if self.active_pane == Pane::Content => {
                self.content.visual_anchor = Some(self.content.cursor);
                self.mode = Mode::VisualSelect;
            }
            KeyCode::Char('L') if self.active_pane == Pane::Content => {
                self.enter_library_browse();
            }
            _ => {}
        }
    }

    fn handle_visual_select_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.content.visual_anchor = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.content.cursor_down();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.content.cursor_up();
            }
            KeyCode::Char('s') => {
                self.title_input.clear();
                self.mode = Mode::TitleInput;
            }
            _ => {}
        }
    }

    fn handle_title_input_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.title_input.clear();
                self.mode = Mode::VisualSelect;
            }
            KeyCode::Enter => {
                self.save_current_snippet();
            }
            KeyCode::Backspace => {
                self.title_input.pop();
            }
            KeyCode::Char(c) => {
                self.title_input.push(c);
            }
            _ => {}
        }
    }

    fn save_current_snippet(&mut self) {
        match crate::library::library_path() {
            Some(path) => self.save_current_snippet_to(&path),
            None => {
                self.status_message = Some("Cannot determine library path.".to_string());
                self.reset_to_normal();
            }
        }
    }

    /// Save snippet to a specific path. Extracted for testability.
    pub fn save_current_snippet_to(&mut self, path: &Path) {
        let title = self.title_input.trim().to_string();
        if title.is_empty() {
            self.status_message = Some("Title cannot be empty.".to_string());
            return;
        }

        let selected_text = match self.content.selected_text() {
            Some(text) => text,
            None => {
                self.status_message = Some("No text selected.".to_string());
                self.reset_to_normal();
                return;
            }
        };

        let source = self.current_source_path();

        let snippet = crate::library::Snippet {
            title,
            content: selected_text,
            source,
        };

        match crate::library::append_snippet(snippet, path) {
            Ok(()) => {
                self.status_message = Some("Snippet saved!".to_string());
            }
            Err(err) => {
                self.status_message = Some(format!("Save failed: {err}"));
            }
        }

        self.reset_to_normal();
    }

    fn enter_library_browse(&mut self) {
        match crate::library::library_path() {
            Some(path) => self.enter_library_browse_from(&path),
            None => {
                self.status_message = Some("Cannot determine library path.".to_string());
            }
        }
    }

    /// Enter library browse with a specific path. Extracted for testability.
    pub fn enter_library_browse_from(&mut self, path: &Path) {
        match crate::library::load_library(path) {
            Ok(lib) => {
                self.library = Some(lib);
                self.library_selected = 0;
                self.mode = Mode::LibraryBrowse;
            }
            Err(err) => {
                self.status_message = Some(format!("Failed to load library: {err}"));
            }
        }
    }

    fn handle_library_browse_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.library = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self
                    .library
                    .as_ref()
                    .map_or(0, |lib| lib.snippets.len().saturating_sub(1));
                if self.library_selected < max {
                    self.library_selected += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.library_selected = self.library_selected.saturating_sub(1);
            }
            KeyCode::Char('d') => {
                self.delete_library_snippet();
            }
            KeyCode::Char('r') => {
                if let Some(lib) = &self.library
                    && let Some(snippet) = lib.snippets.get(self.library_selected)
                {
                    self.title_input = snippet.title.clone();
                    self.mode = Mode::RenameInput;
                }
            }
            _ => {}
        }
    }

    fn handle_rename_input_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.title_input.clear();
                self.mode = Mode::LibraryBrowse;
            }
            KeyCode::Enter => {
                self.rename_library_snippet();
            }
            KeyCode::Backspace => {
                self.title_input.pop();
            }
            KeyCode::Char(c) => {
                self.title_input.push(c);
            }
            _ => {}
        }
    }

    fn rename_library_snippet(&mut self) {
        match crate::library::library_path() {
            Some(path) => self.rename_library_snippet_from(&path),
            None => {
                self.status_message = Some("Cannot determine library path.".to_string());
                self.title_input.clear();
                self.mode = Mode::LibraryBrowse;
            }
        }
    }

    /// Rename a library snippet. Extracted for testability.
    pub fn rename_library_snippet_from(&mut self, path: &Path) {
        let new_title = self.title_input.trim().to_string();
        if new_title.is_empty() {
            self.status_message = Some("Title cannot be empty.".to_string());
            return;
        }

        match crate::library::rename_snippet(self.library_selected, &new_title, path) {
            Ok(()) => {
                if let Ok(lib) = crate::library::load_library(path) {
                    self.library = Some(lib);
                }
                self.status_message = Some("Snippet renamed.".to_string());
            }
            Err(err) => {
                self.status_message = Some(format!("Rename failed: {err}"));
            }
        }

        self.title_input.clear();
        self.mode = Mode::LibraryBrowse;
    }

    fn delete_library_snippet(&mut self) {
        match crate::library::library_path() {
            Some(path) => self.delete_library_snippet_from(&path),
            None => {
                self.status_message = Some("Cannot determine library path.".to_string());
            }
        }
    }

    /// Delete a library snippet at a specific path. Extracted for testability.
    pub fn delete_library_snippet_from(&mut self, path: &Path) {
        let snippet_count = self.library.as_ref().map_or(0, |lib| lib.snippets.len());
        if snippet_count == 0 {
            return;
        }

        match crate::library::delete_snippet(self.library_selected, path) {
            Ok(()) => {
                // Reload library from disk
                if let Ok(lib) = crate::library::load_library(path) {
                    let new_len = lib.snippets.len();
                    self.library = Some(lib);
                    if self.library_selected >= new_len && new_len > 0 {
                        self.library_selected = new_len - 1;
                    } else if new_len == 0 {
                        self.library_selected = 0;
                    }
                }
                self.status_message = Some("Snippet deleted.".to_string());
            }
            Err(err) => {
                self.status_message = Some(format!("Delete failed: {err}"));
            }
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
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyEventKind;
    use ratatui::crossterm::event::KeyEventState;
    use tempfile::TempDir;

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn sample_roots() -> Vec<SourceRoot> {
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

    #[test]
    fn q_key_sets_exit() {
        let mut app = App::new(vec![]);
        app.handle_key_event(key_event(KeyCode::Char('q')));
        assert!(app.exit);
    }

    #[test]
    fn other_keys_do_not_exit() {
        let mut app = App::new(vec![]);
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
    fn tab_toggles_pane() {
        let mut app = App::new(sample_roots());
        assert_eq!(app.active_pane, Pane::FileList);

        app.handle_key_event(key_event(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::Content);

        app.handle_key_event(key_event(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::FileList);
    }

    #[test]
    fn arrow_keys_ignored_when_content_pane_active() {
        let mut app = App::new(sample_roots());
        let initial_selected = app.tree_state.selected().to_vec();

        app.handle_key_event(key_event(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::Content);

        app.handle_key_event(key_event(KeyCode::Down));
        assert_eq!(app.tree_state.selected(), initial_selected);
    }

    #[test]
    fn select_tree_item_on_root_toggles() {
        let mut app = App::new(sample_roots());

        // Directly select a root node (single-segment identifier)
        app.tree_state.select(vec!["/a".to_string()]);

        let initially_opened = app.tree_state.opened().clone();
        assert!(
            initially_opened.contains(&vec!["/a".to_string()]),
            "Root /a should be open initially"
        );

        // Press Enter on a root — should toggle it closed
        app.handle_key_event(key_event(KeyCode::Enter));
        assert!(
            !app.tree_state.opened().contains(&vec!["/a".to_string()]),
            "Root /a should be closed after toggle"
        );

        // Press Enter again — should toggle it open
        app.handle_key_event(key_event(KeyCode::Enter));
        assert!(
            app.tree_state.opened().contains(&vec!["/a".to_string()]),
            "Root /a should be open after second toggle"
        );
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
        let app = App::new(roots);

        // The first file should be auto-selected and its content loaded
        assert_eq!(app.content.text.as_deref(), Some("Test content"));
        assert_eq!(
            app.tree_state.selected(),
            vec![tmp.path().display().to_string(), file.display().to_string()]
        );
    }

    #[test]
    fn select_tree_item_loads_file_content() {
        let tmp = TempDir::new().unwrap();

        let file_a = tmp.path().join("CLAUDE.md");
        fs::write(&file_a, "First content").unwrap();

        let sub = tmp.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        let file_b = sub.join("CLAUDE.md");
        fs::write(&file_b, "Second content").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file_a, file_b.clone()],
        }];
        let mut app = App::new(roots);

        // First file is loaded on startup
        assert_eq!(app.content.text.as_deref(), Some("First content"));

        // Select a different file via tree navigation
        app.tree_state.select(vec![
            tmp.path().display().to_string(),
            file_b.display().to_string(),
        ]);
        app.handle_key_event(key_event(KeyCode::Enter));
        assert_eq!(app.content.text.as_deref(), Some("Second content"));
    }

    #[test]
    fn load_content_handles_missing_file() {
        let roots = vec![SourceRoot {
            path: PathBuf::from("/nonexistent"),
            files: vec![PathBuf::from("/nonexistent/CLAUDE.md")],
        }];
        let mut app = App::new(roots);

        // Directly select the file node
        app.tree_state.select(vec![
            "/nonexistent".to_string(),
            "/nonexistent/CLAUDE.md".to_string(),
        ]);
        app.handle_key_event(key_event(KeyCode::Enter));
        assert!(app.content.text.is_some());
        assert!(
            app.content
                .text
                .as_deref()
                .unwrap()
                .contains("Error reading")
        );
    }

    #[test]
    fn cursor_moves_down_and_scrolls_when_past_viewport() {
        let mut app = App::new(vec![]);
        app.content.text = Some("Line 0\nLine 1\nLine 2\nLine 3\nLine 4".to_string());
        app.content.viewport_height = 3; // can see 3 lines
        app.active_pane = Pane::Content;

        app.handle_key_event(key_event(KeyCode::Down));
        assert_eq!(app.content.cursor, 1);
        assert_eq!(app.content.scroll, 0, "Still visible, no scroll");

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.content.cursor, 2);
        assert_eq!(app.content.scroll, 0, "Line 2 is last visible row");

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.content.cursor, 3);
        assert_eq!(app.content.scroll, 1, "Scrolls to keep cursor visible");
    }

    #[test]
    fn cursor_does_not_go_below_zero() {
        let mut app = App::new(vec![]);
        app.content.text = Some("Line 0\nLine 1".to_string());
        app.active_pane = Pane::Content;

        app.handle_key_event(key_event(KeyCode::Up));
        assert_eq!(app.content.cursor, 0);
    }

    #[test]
    fn cursor_clamps_at_last_line() {
        let mut app = App::new(vec![]);
        app.content.text = Some("Line 0\nLine 1\nLine 2\nLine 3\nLine 4".to_string());
        app.content.viewport_height = 3;
        app.active_pane = Pane::Content;

        app.handle_key_event(key_event(KeyCode::PageDown));
        assert_eq!(app.content.cursor, 3, "Page down moves by viewport_height");

        app.handle_key_event(key_event(KeyCode::PageDown));
        assert_eq!(app.content.cursor, 4, "Clamps at last line");
    }

    #[test]
    fn loading_new_content_resets_scroll_and_cursor() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Line 0\nLine 1\nLine 2").unwrap();

        let root_id = tmp.path().display().to_string();
        let file_id = file.display().to_string();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file],
        }];
        let mut app = App::new(roots);

        // Manually set scroll and cursor
        app.content.scroll = 5;
        app.content.cursor = 5;

        // Load file content via select_tree_item
        app.tree_state
            .select(vec![root_id.clone(), file_id.clone()]);
        app.handle_key_event(key_event(KeyCode::Enter));
        assert_eq!(app.content.scroll, 0, "Loading new content resets scroll");
        assert_eq!(app.content.cursor, 0, "Loading new content resets cursor");
    }

    /// Extract the first content row text from the content pane in the rendered buffer.
    fn extract_content_first_line(buf: &ratatui::buffer::Buffer, width: u16) -> String {
        // Content pane starts at 30% of width; +1 for left border, row 1 is inside top border.
        let content_x_start = (width * 30 / 100) + 1;
        let content_x_end = width - 1; // exclude right border
        (content_x_start..content_x_end)
            .map(|x| buf[(x, 1)].symbol().to_string())
            .collect::<String>()
    }

    #[test]
    fn switching_files_does_not_leave_leftover_characters() {
        let tmp = TempDir::new().unwrap();

        // First file has a long first line
        let dir_a = tmp.path().join("a");
        fs::create_dir_all(&dir_a).unwrap();
        let file_a = dir_a.join("CLAUDE.md");
        fs::write(&file_a, "# CLAUDE.md\nSecond line").unwrap();

        // Second file has a shorter first line
        let dir_b = tmp.path().join("b");
        fs::create_dir_all(&dir_b).unwrap();
        let file_b = dir_b.join("CLAUDE.md");
        fs::write(&file_b, "# Short\nOther").unwrap();

        let roots = vec![
            SourceRoot {
                path: dir_a.clone(),
                files: vec![file_a.clone()],
            },
            SourceRoot {
                path: dir_b.clone(),
                files: vec![file_b.clone()],
            },
        ];
        let mut app = App::new(roots);
        let width: u16 = 80;
        let height: u16 = 10;

        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        // Draw 1: placeholder
        terminal.draw(|frame| app.draw(frame)).unwrap();

        // Load the long file and draw
        app.tree_state.select(vec![
            dir_a.display().to_string(),
            file_a.display().to_string(),
        ]);
        app.load_selected_content();
        terminal.draw(|frame| app.draw(frame)).unwrap();

        let buf = terminal.backend().buffer().clone();
        let line = extract_content_first_line(&buf, width);
        assert_eq!(
            line.trim_end(),
            "# CLAUDE.md",
            "Long file should render correctly"
        );

        // Now switch to the shorter file and draw
        app.tree_state.select(vec![
            dir_b.display().to_string(),
            file_b.display().to_string(),
        ]);
        app.load_selected_content();
        terminal.draw(|frame| app.draw(frame)).unwrap();

        let buf = terminal.backend().buffer().clone();
        let line = extract_content_first_line(&buf, width);
        eprintln!("RAW content row after Draw 3 (# Short): '{line}'");

        // Also check the Terminal's internal buffer directly for comparison
        // The TestBackend buffer should match the screen output
        eprintln!("TestBackend buf cell symbols at row 1, x=25..40:");
        for x in 25u16..40 {
            let sym = buf[(x, 1)].symbol();
            eprint!("[{x}:{}]", sym.escape_debug());
        }
        eprintln!();

        let trimmed = line.trim_end();

        assert_eq!(
            trimmed, "# Short",
            "After switching to shorter file, first line must not have leftover chars. Got: '{trimmed}'"
        );
    }

    #[test]
    fn tabs_are_expanded_to_spaces() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "\tindented\n\t\tdouble").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file.clone()],
        }];
        let mut app = App::new(roots);

        let root_id = tmp.path().display().to_string();
        let file_id = file.display().to_string();
        app.tree_state.select(vec![root_id, file_id]);
        app.handle_key_event(key_event(KeyCode::Enter));

        let content = app.content.text.as_deref().unwrap();
        assert!(
            !content.contains('\t'),
            "Tabs should be replaced with spaces, got: {content:?}"
        );
        assert!(content.starts_with("    indented"));
    }

    // --- ContentState unit tests ---

    #[test]
    fn content_state_selection_range_returns_none_without_anchor() {
        let state = ContentState::new();
        assert_eq!(state.selection_range(), None);
    }

    #[test]
    fn content_state_selection_range_sorts_anchor_and_cursor() {
        let mut state = ContentState::new();
        state.visual_anchor = Some(5);
        state.cursor = 2;
        assert_eq!(state.selection_range(), Some((2, 5)));

        state.cursor = 8;
        assert_eq!(state.selection_range(), Some((5, 8)));
    }

    #[test]
    fn content_state_selected_text_extracts_lines() {
        let mut state = ContentState::new();
        state.text = Some("line 0\nline 1\nline 2\nline 3\nline 4".to_string());
        state.visual_anchor = Some(1);
        state.cursor = 3;

        assert_eq!(
            state.selected_text(),
            Some("line 1\nline 2\nline 3".to_string())
        );
    }

    #[test]
    fn content_state_selected_text_returns_none_without_anchor() {
        let mut state = ContentState::new();
        state.text = Some("line 0\nline 1".to_string());
        assert_eq!(state.selected_text(), None);
    }

    // --- Mode transition tests ---

    #[test]
    fn app_starts_in_normal_mode() {
        let app = App::new(vec![]);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn q_does_not_exit_in_visual_select_mode() {
        let mut app = App::new(vec![]);
        app.mode = Mode::VisualSelect;
        app.handle_key_event(key_event(KeyCode::Char('q')));
        assert!(!app.exit);
    }

    #[test]
    fn q_does_not_exit_in_title_input_mode() {
        let mut app = App::new(vec![]);
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
            Mode::LibraryBrowse,
            Mode::RenameInput,
        ] {
            let mut app = App::new(vec![]);
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
        let mut app = App::new(vec![]);
        app.status_message = Some("Test message".to_string());
        app.handle_key_event(key_event(KeyCode::Char('a')));
        assert!(app.status_message.is_none());
    }

    // --- Visual selection integration tests ---

    #[test]
    fn v_in_content_pane_enters_visual_select() {
        let mut app = App::new(vec![]);
        app.content.text = Some("line 0\nline 1\nline 2".to_string());
        app.active_pane = Pane::Content;
        app.content.cursor = 1;

        app.handle_key_event(key_event(KeyCode::Char('v')));

        assert_eq!(app.mode, Mode::VisualSelect);
        assert_eq!(app.content.visual_anchor, Some(1));
    }

    #[test]
    fn v_in_file_list_does_not_enter_visual_select() {
        let mut app = App::new(vec![]);
        app.active_pane = Pane::FileList;

        app.handle_key_event(key_event(KeyCode::Char('v')));

        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn esc_in_visual_select_returns_to_normal() {
        let mut app = App::new(vec![]);
        app.mode = Mode::VisualSelect;
        app.content.visual_anchor = Some(3);

        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.content.visual_anchor, None);
    }

    #[test]
    fn jk_in_visual_select_moves_cursor() {
        let mut app = App::new(vec![]);
        app.content.text = Some("line 0\nline 1\nline 2\nline 3\nline 4".to_string());
        app.content.viewport_height = 10;
        app.mode = Mode::VisualSelect;
        app.content.visual_anchor = Some(1);
        app.content.cursor = 1;

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.content.cursor, 2);
        assert_eq!(app.content.selection_range(), Some((1, 2)));

        app.handle_key_event(key_event(KeyCode::Char('k')));
        assert_eq!(app.content.cursor, 1);
        assert_eq!(app.content.selection_range(), Some((1, 1)));
    }

    #[test]
    fn s_in_visual_select_enters_title_input() {
        let mut app = App::new(vec![]);
        app.mode = Mode::VisualSelect;
        app.content.visual_anchor = Some(0);

        app.handle_key_event(key_event(KeyCode::Char('s')));

        assert_eq!(app.mode, Mode::TitleInput);
        assert!(app.title_input.is_empty());
    }

    #[test]
    fn loading_new_content_clears_visual_anchor() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "content").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file.clone()],
        }];
        let mut app = App::new(roots);
        app.content.visual_anchor = Some(5);

        // Re-load the same file
        let root_id = tmp.path().display().to_string();
        let file_id = file.display().to_string();
        app.tree_state.select(vec![root_id, file_id]);
        app.handle_key_event(key_event(KeyCode::Enter));

        assert_eq!(app.content.visual_anchor, None);
    }

    // --- Title input integration tests ---

    #[test]
    fn title_input_chars_accumulate() {
        let mut app = App::new(vec![]);
        app.mode = Mode::TitleInput;

        app.handle_key_event(key_event(KeyCode::Char('A')));
        app.handle_key_event(key_event(KeyCode::Char('B')));
        assert_eq!(app.title_input, "AB");
    }

    #[test]
    fn title_input_backspace_deletes_last_char() {
        let mut app = App::new(vec![]);
        app.mode = Mode::TitleInput;
        app.title_input = "ABC".to_string();

        app.handle_key_event(key_event(KeyCode::Backspace));
        assert_eq!(app.title_input, "AB");
    }

    #[test]
    fn title_input_esc_returns_to_visual_select() {
        let mut app = App::new(vec![]);
        app.mode = Mode::TitleInput;
        app.content.visual_anchor = Some(2);
        app.title_input = "partial".to_string();

        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.mode, Mode::VisualSelect);
        assert_eq!(app.content.visual_anchor, Some(2), "Selection preserved");
        assert!(app.title_input.is_empty(), "Input cleared on Esc");
    }

    #[test]
    fn save_with_empty_title_shows_error() {
        let tmp = TempDir::new().unwrap();
        let library_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![]);
        app.mode = Mode::TitleInput;
        app.title_input = "  ".to_string();

        app.save_current_snippet_to(&library_path);

        assert_eq!(app.mode, Mode::TitleInput, "Stays in TitleInput on empty");
        assert!(app.status_message.as_deref().unwrap().contains("empty"),);
    }

    #[test]
    fn title_input_enter_saves_snippet_to_disk() {
        let tmp = TempDir::new().unwrap();
        let library_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![]);
        app.content.text = Some("line 0\nline 1\nline 2\nline 3".to_string());
        app.content.visual_anchor = Some(1);
        app.content.cursor = 2;
        app.mode = Mode::TitleInput;
        app.title_input = "My Snippet".to_string();

        // We can't easily override library_path() in tests, so test the
        // underlying logic via save_current_snippet_to().
        app.save_current_snippet_to(&library_path);

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.content.visual_anchor, None);
        assert!(app.title_input.is_empty());
        assert!(app.status_message.as_deref().unwrap().contains("saved"),);

        // Verify the file was written
        let lib = crate::library::load_library(&library_path).unwrap();
        assert_eq!(lib.snippets.len(), 1);
        assert_eq!(lib.snippets[0].title, "My Snippet");
        assert_eq!(lib.snippets[0].content, "line 1\nline 2");
    }

    #[test]
    fn full_visual_select_to_save_flow() {
        let tmp_content = TempDir::new().unwrap();
        let file = tmp_content.path().join("CLAUDE.md");
        fs::write(&file, "# Rules\n- Rule A\n- Rule B\n- Rule C").unwrap();

        let roots = vec![SourceRoot {
            path: tmp_content.path().to_path_buf(),
            files: vec![file],
        }];
        let mut app = App::new(roots);

        // Switch to content pane
        app.handle_key_event(key_event(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::Content);
        assert_eq!(app.mode, Mode::Normal);

        // Start visual selection at line 0 (scroll = 0)
        app.handle_key_event(key_event(KeyCode::Char('v')));
        assert_eq!(app.mode, Mode::VisualSelect);
        assert_eq!(app.content.visual_anchor, Some(0));

        // Scroll down two lines
        app.handle_key_event(key_event(KeyCode::Char('j')));
        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.content.selection_range(), Some((0, 2)));

        // Press s to enter title input
        app.handle_key_event(key_event(KeyCode::Char('s')));
        assert_eq!(app.mode, Mode::TitleInput);

        // Type a title
        for c in "My Rules".chars() {
            app.handle_key_event(key_event(KeyCode::Char(c)));
        }
        assert_eq!(app.title_input, "My Rules");

        // Save to a temp library path
        let tmp_lib = TempDir::new().unwrap();
        let library_path = tmp_lib.path().join("library.toml");
        app.save_current_snippet_to(&library_path);

        assert_eq!(app.mode, Mode::Normal);

        let lib = crate::library::load_library(&library_path).unwrap();
        assert_eq!(lib.snippets.len(), 1);
        assert_eq!(lib.snippets[0].title, "My Rules");
        assert_eq!(lib.snippets[0].content, "# Rules\n- Rule A\n- Rule B");
    }

    // --- Library browse tests ---

    fn library_with_snippets(path: &std::path::Path, titles: &[&str]) {
        for title in titles {
            crate::library::append_snippet(
                crate::library::Snippet {
                    title: title.to_string(),
                    content: format!("Content of {title}"),
                    source: "/test/CLAUDE.md".to_string(),
                },
                path,
            )
            .unwrap();
        }
    }

    #[test]
    fn l_in_content_pane_enters_library_browse() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["Snippet A"]);

        let mut app = App::new(vec![]);
        app.active_pane = Pane::Content;
        app.enter_library_browse_from(&lib_path);

        assert_eq!(app.mode, Mode::LibraryBrowse);
        assert_eq!(app.library_selected, 0);
        assert!(app.library.is_some());
        assert_eq!(app.library.as_ref().unwrap().snippets.len(), 1);
    }

    #[test]
    fn l_in_file_list_does_not_enter_library_browse() {
        let mut app = App::new(vec![]);
        app.active_pane = Pane::FileList;

        app.handle_key_event(key_event(KeyCode::Char('L')));

        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn esc_in_library_browse_returns_to_normal() {
        let mut app = App::new(vec![]);
        app.mode = Mode::LibraryBrowse;
        app.library = Some(crate::library::SnippetLibrary::default());

        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.mode, Mode::Normal);
        assert!(app.library.is_none(), "Library freed on exit");
    }

    #[test]
    fn q_in_library_browse_returns_to_normal_not_exit() {
        let mut app = App::new(vec![]);
        app.mode = Mode::LibraryBrowse;
        app.library = Some(crate::library::SnippetLibrary::default());

        app.handle_key_event(key_event(KeyCode::Char('q')));

        assert_eq!(app.mode, Mode::Normal);
        assert!(!app.exit, "q should not exit the app from LibraryBrowse");
    }

    #[test]
    fn jk_in_library_browse_navigates() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["A", "B", "C"]);

        let mut app = App::new(vec![]);
        app.enter_library_browse_from(&lib_path);
        assert_eq!(app.library_selected, 0);

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.library_selected, 1);

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.library_selected, 2);

        // Clamp at end
        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.library_selected, 2);

        app.handle_key_event(key_event(KeyCode::Char('k')));
        assert_eq!(app.library_selected, 1);

        // Clamp at start
        app.handle_key_event(key_event(KeyCode::Char('k')));
        app.handle_key_event(key_event(KeyCode::Char('k')));
        assert_eq!(app.library_selected, 0);
    }

    #[test]
    fn d_in_library_browse_deletes_snippet() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["A", "B", "C"]);

        let mut app = App::new(vec![]);
        app.enter_library_browse_from(&lib_path);

        // Select "B" (index 1) and delete it
        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.library_selected, 1);

        app.delete_library_snippet_from(&lib_path);

        assert_eq!(app.library.as_ref().unwrap().snippets.len(), 2);
        assert_eq!(app.library.as_ref().unwrap().snippets[0].title, "A");
        assert_eq!(app.library.as_ref().unwrap().snippets[1].title, "C");
        assert_eq!(app.library_selected, 1, "Selected index stays at 1 (now C)");

        // Verify persisted
        let lib = crate::library::load_library(&lib_path).unwrap();
        assert_eq!(lib.snippets.len(), 2);
    }

    #[test]
    fn d_on_last_item_adjusts_selection() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["A", "B"]);

        let mut app = App::new(vec![]);
        app.enter_library_browse_from(&lib_path);

        // Select last item and delete
        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.library_selected, 1);

        app.delete_library_snippet_from(&lib_path);

        assert_eq!(app.library.as_ref().unwrap().snippets.len(), 1);
        assert_eq!(app.library_selected, 0, "Adjusted to last valid index");
    }

    #[test]
    fn d_on_empty_library_is_noop() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![]);
        app.enter_library_browse_from(&lib_path);

        assert!(app.library.as_ref().unwrap().snippets.is_empty());

        app.delete_library_snippet_from(&lib_path);

        assert!(app.library.as_ref().unwrap().snippets.is_empty());
    }

    #[test]
    fn library_browse_loads_from_disk() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["X", "Y"]);

        let mut app = App::new(vec![]);
        app.enter_library_browse_from(&lib_path);

        let lib = app.library.as_ref().unwrap();
        assert_eq!(lib.snippets.len(), 2);
        assert_eq!(lib.snippets[0].title, "X");
        assert_eq!(lib.snippets[1].title, "Y");
    }

    // --- Rename tests ---

    #[test]
    fn r_in_library_browse_enters_rename_with_current_title() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["My Snippet"]);

        let mut app = App::new(vec![]);
        app.enter_library_browse_from(&lib_path);

        app.handle_key_event(key_event(KeyCode::Char('r')));

        assert_eq!(app.mode, Mode::RenameInput);
        assert_eq!(app.title_input, "My Snippet");
    }

    #[test]
    fn rename_esc_returns_to_library_browse() {
        let mut app = App::new(vec![]);
        app.mode = Mode::RenameInput;
        app.title_input = "partial edit".to_string();

        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.mode, Mode::LibraryBrowse);
        assert!(app.title_input.is_empty());
    }

    #[test]
    fn rename_enter_saves_new_title() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["Old Title"]);

        let mut app = App::new(vec![]);
        app.enter_library_browse_from(&lib_path);
        app.mode = Mode::RenameInput;
        app.title_input = "New Title".to_string();

        app.rename_library_snippet_from(&lib_path);

        assert_eq!(app.mode, Mode::LibraryBrowse);
        assert!(app.title_input.is_empty());
        assert_eq!(app.library.as_ref().unwrap().snippets[0].title, "New Title");

        // Verify persisted
        let lib = crate::library::load_library(&lib_path).unwrap();
        assert_eq!(lib.snippets[0].title, "New Title");
    }

    #[test]
    fn rename_with_empty_title_shows_error() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["Keep Me"]);

        let mut app = App::new(vec![]);
        app.enter_library_browse_from(&lib_path);
        app.mode = Mode::RenameInput;
        app.title_input = "  ".to_string();

        app.rename_library_snippet_from(&lib_path);

        assert_eq!(app.mode, Mode::RenameInput, "Stays in RenameInput on empty");
        assert!(app.status_message.as_deref().unwrap().contains("empty"));

        // Original title preserved
        let lib = crate::library::load_library(&lib_path).unwrap();
        assert_eq!(lib.snippets[0].title, "Keep Me");
    }

    #[test]
    fn r_on_empty_library_is_noop() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![]);
        app.enter_library_browse_from(&lib_path);

        app.handle_key_event(key_event(KeyCode::Char('r')));

        assert_eq!(
            app.mode,
            Mode::LibraryBrowse,
            "Stays in browse on empty lib"
        );
    }
}
