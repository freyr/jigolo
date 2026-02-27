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

#[derive(Debug)]
pub struct App {
    pub exit: bool,
    tree_state: TreeState<TreeId>,
    tree_items: Vec<TreeItem<'static, TreeId>>,
    active_pane: Pane,
    content: Option<String>,
    content_scroll: u16,
    content_line_count: usize,
}

impl App {
    pub fn new(roots: Vec<SourceRoot>) -> Self {
        let tree_items = build_tree_items(&roots);
        let mut tree_state = TreeState::default();

        // Open all root nodes by default
        for root in &roots {
            tree_state.open(vec![root.path.display().to_string()]);
        }

        // Select first item
        tree_state.select_first();

        Self {
            exit: false,
            tree_state,
            tree_items,
            active_pane: Pane::FileList,
            content: None,
            content_scroll: 0,
            content_line_count: 0,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(frame.area());

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

        let display_text = self
            .content
            .as_deref()
            .unwrap_or("Select a file to view its content.");
        let content = Paragraph::new(display_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(content_border_style)
                    .title("Content"),
            )
            .scroll((self.content_scroll, 0));
        frame.render_widget(content, chunks[1]);

        let mut scrollbar_state =
            ScrollbarState::new(self.content_line_count).position(self.content_scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
    }

    fn select_tree_item(&mut self) {
        let selected = self.tree_state.selected();
        if selected.is_empty() {
            return;
        }

        // A root node has exactly one identifier segment; a file has two.
        if selected.len() == 1 {
            self.tree_state.toggle_selected();
        } else {
            let file_path = selected.last().cloned();
            if let Some(path_str) = file_path {
                self.load_file_content(&PathBuf::from(path_str));
            }
        }
    }

    fn load_file_content(&mut self, path: &PathBuf) {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => format!("Error reading {}: {err}", path.display()),
        };
        self.content_line_count = text.lines().count();
        self.content = Some(text);
        self.content_scroll = 0;
    }

    fn max_scroll(&self) -> u16 {
        self.content_line_count.saturating_sub(1) as u16
    }

    fn scroll_up(&mut self, amount: u16) {
        self.content_scroll = self.content_scroll.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: u16) {
        self.content_scroll = self
            .content_scroll
            .saturating_add(amount)
            .min(self.max_scroll());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if let Event::Key(key_event) = event::read()? {
            self.handle_key_event(key_event);
        }
        Ok(())
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit = true,
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.exit = true;
            }
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
            }
            KeyCode::Up | KeyCode::Char('k') if self.active_pane == Pane::FileList => {
                self.tree_state.key_up();
            }
            KeyCode::Left | KeyCode::Char('h') if self.active_pane == Pane::FileList => {
                self.tree_state.key_left();
            }
            KeyCode::Right | KeyCode::Char('l') if self.active_pane == Pane::FileList => {
                self.tree_state.key_right();
            }
            KeyCode::Down | KeyCode::Char('j') if self.active_pane == Pane::Content => {
                self.scroll_down(1);
            }
            KeyCode::Up | KeyCode::Char('k') if self.active_pane == Pane::Content => {
                self.scroll_up(1);
            }
            KeyCode::PageDown if self.active_pane == Pane::Content => {
                self.scroll_down(10);
            }
            KeyCode::PageUp if self.active_pane == Pane::Content => {
                self.scroll_up(10);
            }
            _ => {}
        }
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
    fn select_tree_item_loads_file_content() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Test content").unwrap();

        let root_id = tmp.path().display().to_string();
        let file_id = file.display().to_string();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file],
        }];
        let mut app = App::new(roots);
        assert!(app.content.is_none());

        // Directly select the file node (two-segment identifier: root + file)
        app.tree_state.select(vec![root_id, file_id]);
        app.handle_key_event(key_event(KeyCode::Enter));
        assert_eq!(app.content.as_deref(), Some("Test content"));
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
        assert!(app.content.is_some());
        assert!(app.content.as_deref().unwrap().contains("Error reading"));
    }

    #[test]
    fn scroll_down_increases_offset() {
        let mut app = App::new(vec![]);
        app.content = Some("Line 0\nLine 1\nLine 2\nLine 3\nLine 4".to_string());
        app.content_line_count = 5;
        app.active_pane = Pane::Content;

        app.handle_key_event(key_event(KeyCode::Down));
        assert_eq!(app.content_scroll, 1);

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.content_scroll, 2);
    }

    #[test]
    fn scroll_up_does_not_go_below_zero() {
        let mut app = App::new(vec![]);
        app.content = Some("Line 0\nLine 1".to_string());
        app.content_line_count = 2;
        app.active_pane = Pane::Content;

        app.handle_key_event(key_event(KeyCode::Up));
        assert_eq!(app.content_scroll, 0);
    }

    #[test]
    fn scroll_clamps_at_max() {
        let mut app = App::new(vec![]);
        app.content = Some("Line 0\nLine 1\nLine 2\nLine 3\nLine 4".to_string());
        app.content_line_count = 5;
        app.active_pane = Pane::Content;

        app.handle_key_event(key_event(KeyCode::PageDown));
        assert_eq!(app.content_scroll, 4); // max_scroll = line_count - 1
    }

    #[test]
    fn loading_new_content_resets_scroll() {
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

        // Manually set scroll offset
        app.content_scroll = 5;

        // Load file content via select_tree_item
        app.tree_state
            .select(vec![root_id.clone(), file_id.clone()]);
        app.handle_key_event(key_event(KeyCode::Enter));
        assert_eq!(app.content_scroll, 0, "Loading new content resets scroll");
    }
}
