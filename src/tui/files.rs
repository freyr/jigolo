use std::path::Path;

use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;
use ratatui::widgets::ScrollbarState;

use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;

use tui_tree_widget::Tree;

use super::app::App;
use super::app::Mode;
use super::app::Pane;

impl App {
    pub(crate) fn draw_files_screen(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        // In edit mode, render the full area as an editor
        if self.mode == Mode::Edit {
            self.draw_edit_pane(frame, area);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let file_border_style = if self.active_pane == Pane::FileList {
            self.theme.active_border
        } else {
            self.theme.inactive_border
        };

        let content_border_style = if self.active_pane == Pane::Content {
            self.theme.active_border
        } else {
            self.theme.inactive_border
        };

        if let Ok(tree) = Tree::new(&self.tree_items) {
            let tree = tree
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(file_border_style)
                        .title("CLAUDE.md files"),
                )
                .highlight_style(self.theme.highlight);
            frame.render_stateful_widget(tree, chunks[0], &mut self.tree_state);
        }

        self.draw_content_pane(frame, chunks[1], content_border_style);
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
        let cursor_style = self.theme.highlight;
        let highlight_style = self.theme.visual_selection;

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
                    style = style.add_modifier(Modifier::REVERSED);
                    if selection.is_none() {
                        style = cursor_style;
                    }
                }
                // Ensure the cursor line has at least a space so the
                // REVERSED style is visible even on empty lines.
                let text = if show_cursor && i == cursor_line && line_text.is_empty() {
                    " ".to_string()
                } else {
                    line_text.to_string()
                };
                Line::from(text).style(style)
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

    pub(crate) fn handle_normal_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit = true,
            KeyCode::Tab => {
                self.active_pane = match self.active_pane {
                    Pane::FileList => Pane::Content,
                    Pane::Content => Pane::FileList,
                };
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
                let before = self.tree_state.selected().to_vec();
                self.tree_state.key_left();
                if self.tree_state.selected().is_empty() {
                    self.tree_state.select(before);
                }
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
            KeyCode::Char('e') if self.active_pane == Pane::Content => {
                self.enter_edit_mode();
            }
            _ => {}
        }
    }

    pub(crate) fn handle_visual_select_key(&mut self, key_event: KeyEvent) {
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
                self.title_cursor = 0;
                self.mode = Mode::TitleInput;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_title_input_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.title_input.clear();
                self.title_cursor = 0;
                self.mode = Mode::VisualSelect;
            }
            KeyCode::Enter => {
                self.save_current_snippet();
            }
            KeyCode::Backspace => {
                if self.title_cursor > 0 {
                    self.title_cursor -= 1;
                    self.title_input.remove(self.title_cursor);
                }
            }
            KeyCode::Left => {
                self.title_cursor = self.title_cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                if self.title_cursor < self.title_input.len() {
                    self.title_cursor += 1;
                }
            }
            KeyCode::Char(c) => {
                self.title_input.insert(self.title_cursor, c);
                self.title_cursor += 1;
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
                self.compose_state = None;
            }
            Err(err) => {
                self.status_message = Some(format!("Save failed: {err}"));
            }
        }

        self.reset_to_normal();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyCode;
    use ratatui::style::Modifier;

    use tempfile::TempDir;

    use crate::config::Config;
    use crate::model::SourceRoot;
    use crate::tui::app::App;
    use crate::tui::app::Mode;
    use crate::tui::app::Pane;
    use crate::tui::app::test_helpers::key_event;
    use crate::tui::app::test_helpers::render_once;
    use crate::tui::app::test_helpers::sample_roots;

    #[test]
    fn tab_toggles_pane() {
        let mut app = App::new(sample_roots(), &Config::default());
        assert_eq!(app.active_pane, Pane::FileList);

        app.handle_key_event(key_event(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::Content);

        app.handle_key_event(key_event(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::FileList);
    }

    #[test]
    fn arrow_keys_ignored_when_content_pane_active() {
        let mut app = App::new(sample_roots(), &Config::default());
        let initial_selected = app.tree_state.selected().to_vec();

        app.handle_key_event(key_event(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::Content);

        app.handle_key_event(key_event(KeyCode::Down));
        assert_eq!(app.tree_state.selected(), initial_selected);
    }

    #[test]
    fn jk_can_land_on_folder_node() {
        let mut app = App::new(sample_roots(), &Config::default());
        render_once(&mut app);

        // App starts on first file /a/CLAUDE.md -- selected len is 2
        assert_eq!(app.tree_state.selected().len(), 2);

        // Press k (up) -- should land on the /a folder node (len 1)
        app.handle_key_event(key_event(KeyCode::Char('k')));
        assert_eq!(
            app.tree_state.selected().len(),
            1,
            "k should be able to land on a folder node"
        );
    }

    #[test]
    fn folder_selection_clears_content_pane() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Some content").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file],
        }];
        let mut app = App::new(roots, &Config::default());

        // Content is loaded on startup
        assert!(app.content.text.is_some());

        // Select the root/folder node
        app.tree_state
            .select(vec![tmp.path().display().to_string()]);
        app.load_selected_content();

        assert!(
            app.content.text.is_none(),
            "Content pane should be cleared when a folder is selected"
        );
    }

    #[test]
    fn left_arrow_to_parent_clears_content() {
        let mut app = App::new(sample_roots(), &Config::default());
        render_once(&mut app);

        // Start on first file -- content is loaded
        assert_eq!(app.tree_state.selected().len(), 2);
        assert!(app.content.text.is_some());

        // Press Left -- should navigate to parent folder
        app.handle_key_event(key_event(KeyCode::Left));

        assert_eq!(
            app.tree_state.selected().len(),
            1,
            "Left should navigate to parent folder"
        );
        assert!(
            app.content.text.is_none(),
            "Content should be cleared when folder is selected via Left"
        );
    }

    #[test]
    fn left_on_folder_node_does_not_lose_selection() {
        let mut app = App::new(sample_roots(), &Config::default());
        render_once(&mut app);

        // Navigate to the /a folder node
        app.tree_state.select(vec!["/a".to_string()]);
        assert_eq!(app.tree_state.selected().len(), 1);

        // First Left closes the folder (it starts open) -- stays on folder
        app.handle_key_event(key_event(KeyCode::Left));
        assert_eq!(
            app.tree_state.selected().len(),
            1,
            "First Left should close folder, selection stays"
        );

        // Second Left on a closed folder -- selection must not become empty
        app.handle_key_event(key_event(KeyCode::Left));
        assert!(
            !app.tree_state.selected().is_empty(),
            "Second Left on a closed folder should not clear the selection"
        );
        assert_eq!(
            app.tree_state.selected().len(),
            1,
            "Selection should remain on the folder node"
        );
    }

    #[test]
    fn cursor_on_empty_line_is_visible() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        // File with an empty second line
        fs::write(&file, "first\n\nthird").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file],
        }];
        let mut app = App::new(roots, &Config::default());
        app.active_pane = Pane::Content;

        // Move cursor to the empty line (line index 1)
        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.content.cursor, 1);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let buf = terminal.backend().buffer().clone();

        // The cursor line is row 3 in the buffer (row 0 = tab bar, row 1 = border,
        // row 2 = first content line, row 3 = empty cursor line).
        // Check that the empty line has a non-default style (Reversed modifier).
        let content_x_start = (80u16 * 30 / 100) + 1;
        let cell = &buf[(content_x_start, 3)];
        assert!(
            cell.modifier.contains(Modifier::REVERSED),
            "Empty cursor line should use REVERSED style for visibility, got: {:?}",
            cell.modifier
        );
    }

    #[test]
    fn enter_in_file_list_is_noop() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Test content").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file.clone()],
        }];
        let mut app = App::new(roots, &Config::default());

        // Snapshot state before pressing Enter
        let pane_before = app.active_pane;
        let mode_before = app.mode;
        let content_before = app.content.text.clone();
        let selected_before = app.tree_state.selected().to_vec();

        // Press Enter on a file node -- should be a no-op
        app.handle_key_event(key_event(KeyCode::Enter));

        assert_eq!(
            app.active_pane, pane_before,
            "Enter should not change active pane"
        );
        assert_eq!(app.mode, mode_before, "Enter should not change mode");
        assert_eq!(
            app.content.text, content_before,
            "Enter should not reload content"
        );
        assert_eq!(
            app.tree_state.selected(),
            selected_before,
            "Enter should not change selection"
        );
    }

    #[test]
    fn enter_on_root_node_is_noop() {
        let mut app = App::new(sample_roots(), &Config::default());

        // Select a root node
        app.tree_state.select(vec!["/a".to_string()]);
        let opened_before = app.tree_state.opened().clone();

        // Press Enter -- should not toggle the folder
        app.handle_key_event(key_event(KeyCode::Enter));

        assert_eq!(
            app.tree_state.opened().clone(),
            opened_before,
            "Enter should not toggle folder open/closed"
        );
    }

    #[test]
    fn toggle_selected_on_root_toggles() {
        let mut app = App::new(sample_roots(), &Config::default());

        // Directly select a root node (single-segment identifier)
        app.tree_state.select(vec!["/a".to_string()]);

        let initially_opened = app.tree_state.opened().clone();
        assert!(
            initially_opened.contains(&vec!["/a".to_string()]),
            "Root /a should be open initially"
        );

        // Toggle via tree_state directly -- should close
        app.tree_state.toggle_selected();
        assert!(
            !app.tree_state.opened().contains(&vec!["/a".to_string()]),
            "Root /a should be closed after toggle"
        );

        // Toggle again -- should open
        app.tree_state.toggle_selected();
        assert!(
            app.tree_state.opened().contains(&vec!["/a".to_string()]),
            "Root /a should be open after second toggle"
        );
    }

    #[test]
    fn load_selected_content_loads_file() {
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
        let mut app = App::new(roots, &Config::default());

        // First file is loaded on startup
        assert_eq!(app.content.text.as_deref(), Some("First content"));

        // Select a different file and load content directly
        app.tree_state.select(vec![
            tmp.path().display().to_string(),
            file_b.display().to_string(),
        ]);
        app.load_selected_content();
        assert_eq!(app.content.text.as_deref(), Some("Second content"));
    }

    #[test]
    fn load_content_handles_missing_file() {
        let roots = vec![SourceRoot {
            path: PathBuf::from("/nonexistent"),
            files: vec![PathBuf::from("/nonexistent/CLAUDE.md")],
        }];
        let mut app = App::new(roots, &Config::default());

        // Directly select the file node and load content
        app.tree_state.select(vec![
            "/nonexistent".to_string(),
            "/nonexistent/CLAUDE.md".to_string(),
        ]);
        app.load_selected_content();
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
        let mut app = App::new(vec![], &Config::default());
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
        let mut app = App::new(vec![], &Config::default());
        app.content.text = Some("Line 0\nLine 1".to_string());
        app.active_pane = Pane::Content;

        app.handle_key_event(key_event(KeyCode::Up));
        assert_eq!(app.content.cursor, 0);
    }

    #[test]
    fn cursor_clamps_at_last_line() {
        let mut app = App::new(vec![], &Config::default());
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
        let mut app = App::new(roots, &Config::default());

        // Manually set scroll and cursor
        app.content.scroll = 5;
        app.content.cursor = 5;

        // Load file content directly
        app.tree_state
            .select(vec![root_id.clone(), file_id.clone()]);
        app.load_selected_content();
        assert_eq!(app.content.scroll, 0, "Loading new content resets scroll");
        assert_eq!(app.content.cursor, 0, "Loading new content resets cursor");
    }

    /// Extract the first content row text from the content pane in the rendered buffer.
    fn extract_content_first_line(buf: &ratatui::buffer::Buffer, width: u16) -> String {
        // Row 0 = tab bar, row 1 = border top of content pane,
        // row 2 = first content line inside the border.
        let content_x_start = (width * 30 / 100) + 1;
        let content_x_end = width - 1; // exclude right border
        (content_x_start..content_x_end)
            .map(|x| buf[(x, 2)].symbol().to_string())
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
        let mut app = App::new(roots, &Config::default());
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
        eprintln!("TestBackend buf cell symbols at row 2, x=25..40:");
        for x in 25u16..40 {
            let sym = buf[(x, 2)].symbol();
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
        let mut app = App::new(roots, &Config::default());

        let root_id = tmp.path().display().to_string();
        let file_id = file.display().to_string();
        app.tree_state.select(vec![root_id, file_id]);
        app.load_selected_content();

        let content = app.content.text.as_deref().unwrap();
        assert!(
            !content.contains('\t'),
            "Tabs should be replaced with spaces, got: {content:?}"
        );
        assert!(content.starts_with("    indented"));
    }

    // --- ContentState unit tests ---

    use crate::tui::app::ContentState;

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

    // --- Visual selection integration tests ---

    #[test]
    fn v_in_content_pane_enters_visual_select() {
        let mut app = App::new(vec![], &Config::default());
        app.content.text = Some("line 0\nline 1\nline 2".to_string());
        app.active_pane = Pane::Content;
        app.content.cursor = 1;

        app.handle_key_event(key_event(KeyCode::Char('v')));

        assert_eq!(app.mode, Mode::VisualSelect);
        assert_eq!(app.content.visual_anchor, Some(1));
    }

    #[test]
    fn v_in_file_list_does_not_enter_visual_select() {
        let mut app = App::new(vec![], &Config::default());
        app.active_pane = Pane::FileList;

        app.handle_key_event(key_event(KeyCode::Char('v')));

        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn esc_in_visual_select_returns_to_normal() {
        let mut app = App::new(vec![], &Config::default());
        app.mode = Mode::VisualSelect;
        app.content.visual_anchor = Some(3);

        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.content.visual_anchor, None);
    }

    #[test]
    fn jk_in_visual_select_moves_cursor() {
        let mut app = App::new(vec![], &Config::default());
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
        let mut app = App::new(vec![], &Config::default());
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
        let mut app = App::new(roots, &Config::default());
        app.content.visual_anchor = Some(5);

        // Re-load the same file
        let root_id = tmp.path().display().to_string();
        let file_id = file.display().to_string();
        app.tree_state.select(vec![root_id, file_id]);
        app.load_selected_content();

        assert_eq!(app.content.visual_anchor, None);
    }

    // --- Title input integration tests ---

    #[test]
    fn title_input_chars_accumulate() {
        let mut app = App::new(vec![], &Config::default());
        app.mode = Mode::TitleInput;

        app.handle_key_event(key_event(KeyCode::Char('A')));
        app.handle_key_event(key_event(KeyCode::Char('B')));
        assert_eq!(app.title_input, "AB");
    }

    #[test]
    fn title_input_backspace_deletes_at_cursor() {
        let mut app = App::new(vec![], &Config::default());
        app.mode = Mode::TitleInput;
        app.title_input = "ABC".to_string();
        app.title_cursor = 3;

        app.handle_key_event(key_event(KeyCode::Backspace));
        assert_eq!(app.title_input, "AB");
        assert_eq!(app.title_cursor, 2);
    }

    #[test]
    fn title_input_esc_returns_to_visual_select() {
        let mut app = App::new(vec![], &Config::default());
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

        let mut app = App::new(vec![], &Config::default());
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

        let mut app = App::new(vec![], &Config::default());
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
        let mut app = App::new(roots, &Config::default());

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
}
