use std::fs;
use std::io;
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
}

#[derive(Debug)]
pub struct ContentState {
    pub text: Option<String>,
    pub scroll: u16,
    pub cursor: usize,
    pub line_count: usize,
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
            line_count: 0,
            visual_anchor: None,
            viewport_height: 0,
        }
    }

    fn max_cursor(&self) -> usize {
        self.line_count.saturating_sub(1)
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
        self.line_count = text.lines().count();
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
pub struct TitleInputState {
    pub buffer: String,
}

impl TitleInputState {
    pub fn insert_char(&mut self, c: char) {
        self.buffer.push(c);
    }

    pub fn delete_char(&mut self) {
        self.buffer.pop();
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
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
    pub title_input: TitleInputState,
    pub status_message: Option<String>,
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
            title_input: TitleInputState::default(),
            status_message: None,
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
        let has_input_or_status = self.mode == Mode::TitleInput || self.status_message.is_some();
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

        let tree = Tree::new(&self.tree_items)
            .expect("tree items have unique identifiers")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(file_border_style)
                    .title("CLAUDE.md files"),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(tree, chunks[0], &mut self.tree_state);

        let content_title = match self.mode {
            Mode::VisualSelect | Mode::TitleInput => {
                if let Some((start, end)) = self.content.selection_range() {
                    format!("Content [VISUAL: lines {}-{}]", start + 1, end + 1)
                } else {
                    "Content [VISUAL]".to_string()
                }
            }
            Mode::Normal => "Content".to_string(),
        };

        // Capture viewport height (content area minus 2 for borders)
        self.content.viewport_height = chunks[1].height.saturating_sub(2);

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
        let content_widget = Paragraph::new(Text::from(lines));

        let content_widget = content_widget
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(content_border_style)
                    .title(content_title),
            )
            .scroll((self.content.scroll, 0));
        frame.render_widget(content_widget, chunks[1]);

        let mut scrollbar_state =
            ScrollbarState::new(self.content.line_count).position(self.content.scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);

        // Input/status bar (when active)
        if has_input_or_status {
            let bar_area = vertical[1];
            if self.mode == Mode::TitleInput {
                let input_widget = Paragraph::new(self.title_input.buffer.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow))
                        .title("Snippet title"),
                );
                frame.render_widget(input_widget, bar_area);
                let cursor_x = bar_area.x + 1 + self.title_input.buffer.len() as u16;
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

    fn load_file_content(&mut self, path: &PathBuf) {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => format!("Error reading {}: {err}", path.display()),
        };
        self.content.load_text(text);
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
                self.mode = Mode::TitleInput;
                self.title_input.clear();
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
                self.title_input.delete_char();
            }
            KeyCode::Char(c) => {
                self.title_input.insert_char(c);
            }
            _ => {}
        }
    }

    fn save_current_snippet(&mut self) {
        match crate::library::library_path() {
            Some(path) => self.save_current_snippet_to(&path),
            None => {
                self.status_message = Some("Cannot determine library path.".to_string());
                self.mode = Mode::Normal;
                self.content.visual_anchor = None;
                self.title_input.clear();
            }
        }
    }

    /// Save snippet to a specific path. Extracted for testability.
    pub fn save_current_snippet_to(&mut self, path: &std::path::Path) {
        let title = self.title_input.buffer.trim().to_string();
        if title.is_empty() {
            self.status_message = Some("Title cannot be empty.".to_string());
            return;
        }

        let selected_text = match self.content.selected_text() {
            Some(text) => text,
            None => {
                self.status_message = Some("No text selected.".to_string());
                self.mode = Mode::Normal;
                self.content.visual_anchor = None;
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

        self.mode = Mode::Normal;
        self.content.visual_anchor = None;
        self.title_input.clear();
    }
}

pub fn build_tree_items(roots: &[SourceRoot]) -> Vec<TreeItem<'static, TreeId>> {
    roots
        .iter()
        .map(|root| {
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
            TreeItem::new(root_id, root.path.display().to_string(), children)
                .expect("file paths are unique within a root")
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
        app.content.line_count = 5;
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
        app.content.line_count = 2;
        app.active_pane = Pane::Content;

        app.handle_key_event(key_event(KeyCode::Up));
        assert_eq!(app.content.cursor, 0);
    }

    #[test]
    fn cursor_clamps_at_last_line() {
        let mut app = App::new(vec![]);
        app.content.text = Some("Line 0\nLine 1\nLine 2\nLine 3\nLine 4".to_string());
        app.content.line_count = 5;
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
        state.line_count = 5;
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
        for mode in [Mode::Normal, Mode::VisualSelect, Mode::TitleInput] {
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

    // --- TitleInputState unit tests ---

    #[test]
    fn title_input_insert_and_delete() {
        let mut input = TitleInputState::default();
        input.insert_char('h');
        input.insert_char('i');
        assert_eq!(input.buffer, "hi");

        input.delete_char();
        assert_eq!(input.buffer, "h");

        input.clear();
        assert_eq!(input.buffer, "");
    }

    #[test]
    fn title_input_delete_on_empty_is_noop() {
        let mut input = TitleInputState::default();
        input.delete_char();
        assert_eq!(input.buffer, "");
    }

    // --- Visual selection integration tests ---

    #[test]
    fn v_in_content_pane_enters_visual_select() {
        let mut app = App::new(vec![]);
        app.content.text = Some("line 0\nline 1\nline 2".to_string());
        app.content.line_count = 3;
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
        app.content.line_count = 5;
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
        assert!(app.title_input.buffer.is_empty());
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
        assert_eq!(app.title_input.buffer, "AB");
    }

    #[test]
    fn title_input_backspace_deletes_last_char() {
        let mut app = App::new(vec![]);
        app.mode = Mode::TitleInput;
        app.title_input.buffer = "ABC".to_string();

        app.handle_key_event(key_event(KeyCode::Backspace));
        assert_eq!(app.title_input.buffer, "AB");
    }

    #[test]
    fn title_input_esc_returns_to_visual_select() {
        let mut app = App::new(vec![]);
        app.mode = Mode::TitleInput;
        app.content.visual_anchor = Some(2);
        app.title_input.buffer = "partial".to_string();

        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.mode, Mode::VisualSelect);
        assert_eq!(app.content.visual_anchor, Some(2), "Selection preserved");
        assert!(app.title_input.buffer.is_empty(), "Input cleared on Esc");
    }

    #[test]
    fn save_with_empty_title_shows_error() {
        let tmp = TempDir::new().unwrap();
        let library_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![]);
        app.mode = Mode::TitleInput;
        app.title_input.buffer = "  ".to_string();

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
        app.content.line_count = 4;
        app.content.visual_anchor = Some(1);
        app.content.cursor = 2;
        app.mode = Mode::TitleInput;
        app.title_input.buffer = "My Snippet".to_string();

        // We can't easily override library_path() in tests, so test the
        // underlying logic via save_current_snippet_to().
        app.save_current_snippet_to(&library_path);

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.content.visual_anchor, None);
        assert!(app.title_input.buffer.is_empty());
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
        assert_eq!(app.title_input.buffer, "My Rules");

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
}
