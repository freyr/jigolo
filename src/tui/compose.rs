use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;
use ratatui::widgets::ScrollbarState;

use super::app::App;
use super::app::Mode;
use super::app::Screen;

/// Which pane is focused on the Compose screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposePane {
    /// Snippet list (left pane).
    List,
    /// Preview (right pane).
    Preview,
}

/// State for the Compose screen.
#[derive(Debug)]
pub struct ComposeState {
    /// Ordered list of selected snippet indices (insertion order = compose order).
    pub selected: Vec<usize>,
    /// Cursor position in the snippet list.
    pub cursor: usize,
    /// Scroll offset for the snippet list pane.
    pub scroll: u16,
    /// Viewport height for the list pane (set during draw).
    pub viewport_height: u16,
    /// Which pane has focus.
    pub active_pane: ComposePane,
    /// Scroll offset for the preview pane.
    pub preview_scroll: u16,
    /// Viewport height for preview pane (set during draw).
    pub preview_viewport_height: u16,
}

impl Default for ComposeState {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposeState {
    /// Creates a new compose state with no selections.
    pub fn new() -> Self {
        Self {
            selected: Vec::new(),
            cursor: 0,
            scroll: 0,
            viewport_height: 0,
            active_pane: ComposePane::List,
            preview_scroll: 0,
            preview_viewport_height: 0,
        }
    }

    /// Returns the number of snippets in the library.
    fn snippet_count(app: &App) -> usize {
        app.library.as_ref().map_or(0, |lib| lib.snippets.len())
    }

    /// Checks whether a given snippet index is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    /// Toggles a snippet: appends if not selected, removes if already selected.
    pub fn toggle(&mut self, index: usize) {
        if let Some(pos) = self.selected.iter().position(|&i| i == index) {
            self.selected.remove(pos);
        } else {
            self.selected.push(index);
        }
    }
}

impl App {
    /// Enters the Compose screen, loading the library if needed.
    pub(crate) fn enter_compose_screen(&mut self) {
        if self.library.is_none() {
            if let Some(path) = crate::library::library_path() {
                match crate::library::load_library(&path) {
                    Ok(lib) => self.library = Some(lib),
                    Err(err) => {
                        self.status_message = Some(format!("Failed to load library: {err}"));
                        return;
                    }
                }
            } else {
                self.status_message = Some("Cannot determine library path.".to_string());
                return;
            }
        }

        if self.compose_state.is_none() {
            self.compose_state = Some(ComposeState::new());
        }

        self.screen = Screen::Compose;
    }

    /// Enters the Compose screen with a specific library path (for testability).
    pub fn enter_compose_screen_from(&mut self, path: &std::path::Path) {
        match crate::library::load_library(path) {
            Ok(lib) => {
                self.library = Some(lib);
                if self.compose_state.is_none() {
                    self.compose_state = Some(ComposeState::new());
                }
                self.screen = Screen::Compose;
            }
            Err(err) => {
                self.status_message = Some(format!("Failed to load library: {err}"));
            }
        }
    }

    pub(crate) fn draw_compose_screen(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let library = match &self.library {
            Some(lib) => lib,
            None => {
                let msg = Paragraph::new("Library not loaded.")
                    .block(Block::default().borders(Borders::ALL).title("Compose"));
                frame.render_widget(msg, area);
                return;
            }
        };

        let compose = match &self.compose_state {
            Some(s) => s,
            None => return,
        };

        if library.snippets.is_empty() {
            let msg = Paragraph::new(
                "Library is empty. Save snippets with v then s on the Files screen.",
            )
            .block(Block::default().borders(Borders::ALL).title("Compose"));
            frame.render_widget(msg, area);
            return;
        }

        // Dual-pane layout: snippet list (40%) | preview (60%)
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        let list_area = panes[0];
        let preview_area = panes[1];

        // --- Left pane: snippet list with checkboxes ---
        let list_viewport_height = list_area.height.saturating_sub(2);
        let cursor = compose.cursor;
        let highlight = self.theme.highlight;
        let list_focused = compose.active_pane == ComposePane::List;

        let lines: Vec<Line> = library
            .snippets
            .iter()
            .enumerate()
            .map(|(i, snippet)| {
                let checkbox = if compose.is_selected(i) {
                    "[x] "
                } else {
                    "[ ] "
                };
                let text = format!("{checkbox}{}", snippet.title);
                let style = if i == cursor && list_focused {
                    highlight
                } else {
                    Style::default()
                };
                Line::from(text).style(style)
            })
            .collect();

        let selected_count = compose.selected.len();
        let total_count = library.snippets.len();
        let list_title = format!("Snippets ({selected_count}/{total_count} selected)");

        let list_border_style = if list_focused {
            self.theme.active_border
        } else {
            Style::default()
        };

        let list_widget = Paragraph::new(Text::from(lines))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(list_border_style)
                    .title(list_title),
            )
            .scroll((compose.scroll, 0));
        frame.render_widget(list_widget, list_area);

        let mut list_scrollbar_state =
            ScrollbarState::new(library.snippets.len()).position(compose.scroll as usize);
        let list_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(list_scrollbar, list_area, &mut list_scrollbar_state);

        // --- Right pane: live preview ---
        let preview_viewport_height = preview_area.height.saturating_sub(2);
        let composed = self.composed_text();

        let preview_focused = compose.active_pane == ComposePane::Preview;
        let preview_border_style = if preview_focused {
            self.theme.active_border
        } else {
            Style::default()
        };

        let preview_title = if composed.is_empty() {
            "Preview".to_string()
        } else {
            let line_count = composed.lines().count();
            format!("Preview ({line_count} lines)")
        };

        let preview_widget = Paragraph::new(composed.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(preview_border_style)
                    .title(preview_title),
            )
            .scroll((compose.preview_scroll, 0));
        frame.render_widget(preview_widget, preview_area);

        let preview_line_count = composed.lines().count();
        let mut preview_scrollbar_state =
            ScrollbarState::new(preview_line_count).position(compose.preview_scroll as usize);
        let preview_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(
            preview_scrollbar,
            preview_area,
            &mut preview_scrollbar_state,
        );

        // Update viewport heights
        if let Some(cs) = &mut self.compose_state {
            cs.viewport_height = list_viewport_height;
            cs.preview_viewport_height = preview_viewport_height;
        }
    }

    /// Returns the composed text from currently selected snippets.
    pub(crate) fn composed_text(&self) -> String {
        let library = match &self.library {
            Some(lib) => lib,
            None => return String::new(),
        };
        let compose = match &self.compose_state {
            Some(s) => s,
            None => return String::new(),
        };
        crate::compose::compose_snippets(&library.snippets, &compose.selected)
    }

    pub(crate) fn handle_compose_key(&mut self, key_event: KeyEvent) {
        let compose = match &self.compose_state {
            Some(s) => s,
            None => return,
        };

        let snippet_count = ComposeState::snippet_count(self);
        if snippet_count == 0 {
            match key_event.code {
                KeyCode::Esc => {
                    self.screen = Screen::Files;
                }
                KeyCode::Char('q') => {
                    self.exit = true;
                }
                _ => {}
            }
            return;
        }

        // Route based on active pane
        if compose.active_pane == ComposePane::Preview {
            self.handle_compose_preview_key(key_event);
            return;
        }

        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(cs) = &mut self.compose_state
                    && cs.cursor < snippet_count.saturating_sub(1)
                {
                    cs.cursor += 1;
                    ensure_compose_cursor_visible(cs);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(cs) = &mut self.compose_state {
                    cs.cursor = cs.cursor.saturating_sub(1);
                    ensure_compose_cursor_visible(cs);
                }
            }
            KeyCode::Char(' ') => {
                if let Some(cs) = &mut self.compose_state {
                    let cursor = cs.cursor;
                    cs.toggle(cursor);
                }
            }
            KeyCode::Tab => {
                if let Some(cs) = &mut self.compose_state {
                    cs.active_pane = ComposePane::Preview;
                }
            }
            KeyCode::Char('w') => {
                if let Some(cs) = &self.compose_state {
                    if cs.selected.is_empty() {
                        self.status_message = Some("No snippets selected.".to_string());
                    } else {
                        self.mode = Mode::ExportPath;
                        self.title_input.clear();
                        self.title_cursor = 0;
                    }
                }
            }
            KeyCode::Esc => {
                self.screen = Screen::Files;
            }
            KeyCode::Char('q') => {
                self.exit = true;
            }
            _ => {}
        }
    }

    fn handle_compose_preview_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let line_count = self.composed_text().lines().count();
                if let Some(cs) = &mut self.compose_state {
                    let max_scroll = line_count.saturating_sub(cs.preview_viewport_height as usize);
                    if (cs.preview_scroll as usize) < max_scroll {
                        cs.preview_scroll += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(cs) = &mut self.compose_state {
                    cs.preview_scroll = cs.preview_scroll.saturating_sub(1);
                }
            }
            KeyCode::Tab => {
                if let Some(cs) = &mut self.compose_state {
                    cs.active_pane = ComposePane::List;
                }
            }
            KeyCode::Esc => {
                self.screen = Screen::Files;
            }
            KeyCode::Char('q') => {
                self.exit = true;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_export_path_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Enter => {
                self.execute_export();
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.title_input.clear();
                self.title_cursor = 0;
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

    fn execute_export(&mut self) {
        let raw_path = self.title_input.trim().to_string();
        if raw_path.is_empty() {
            self.status_message = Some("No path entered.".to_string());
            return;
        }

        let expanded = if raw_path.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                raw_path.replacen('~', &home, 1)
            } else {
                self.status_message = Some("Cannot expand ~: HOME not set.".to_string());
                return;
            }
        } else {
            raw_path
        };

        let path = std::path::PathBuf::from(&expanded);

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            self.status_message = Some("Parent directory does not exist.".to_string());
            return;
        }

        if path.exists() {
            self.status_message = Some("File already exists.".to_string());
            return;
        }

        let composed = self.composed_text();
        let selected_count = self
            .compose_state
            .as_ref()
            .map_or(0, |cs| cs.selected.len());

        let parent = path.parent().unwrap_or(std::path::Path::new("."));
        let result = tempfile::NamedTempFile::new_in(parent).and_then(|mut tmp| {
            use std::io::Write;
            tmp.write_all(composed.as_bytes())?;
            tmp.flush()?;
            tmp.persist(&path).map_err(|e| e.error)?;
            Ok(())
        });

        match result {
            Ok(()) => {
                self.status_message = Some(format!(
                    "Exported {selected_count} snippet{} to {}",
                    if selected_count == 1 { "" } else { "s" },
                    path.display()
                ));
                self.mode = Mode::Normal;
                self.title_input.clear();
                self.title_cursor = 0;
            }
            Err(err) => {
                self.status_message = Some(format!("Export failed: {err}"));
            }
        }
    }
}

/// Ensures the compose cursor stays within the visible viewport.
fn ensure_compose_cursor_visible(cs: &mut ComposeState) {
    let scroll = cs.scroll as usize;
    let vh = cs.viewport_height as usize;
    if cs.cursor < scroll {
        cs.scroll = cs.cursor as u16;
    } else if vh > 0 && cs.cursor >= scroll + vh {
        cs.scroll = (cs.cursor - vh + 1) as u16;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ratatui::crossterm::event::KeyCode;

    use tempfile::TempDir;

    use crate::config::Config;
    use crate::library::Snippet;
    use crate::library::SnippetLibrary;
    use crate::tui::app::App;
    use crate::tui::app::Mode;
    use crate::tui::app::Screen;
    use crate::tui::app::test_helpers::key_event;
    use crate::tui::app::test_helpers::render_once;

    use super::ComposePane;

    fn app_with_library(snippets: Vec<(&str, &str)>) -> App {
        let mut app = App::new(vec![], &Config::default());
        app.library = Some(SnippetLibrary {
            snippets: snippets
                .into_iter()
                .map(|(title, content)| Snippet {
                    title: title.to_string(),
                    content: content.to_string(),
                    source: String::new(),
                })
                .collect(),
        });
        app.screen = Screen::Compose;
        app.compose_state = Some(super::ComposeState::new());
        app
    }

    #[test]
    fn key_3_enters_compose_screen() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        crate::library::save_library(&SnippetLibrary::default(), &lib_path).unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_compose_screen_from(&lib_path);

        assert_eq!(app.screen, Screen::Compose);
        assert!(app.compose_state.is_some());
    }

    #[test]
    fn compose_empty_library_shows_message() {
        let mut app = app_with_library(vec![]);
        render_once(&mut app);
    }

    #[test]
    fn jk_navigates_snippet_list() {
        let mut app = app_with_library(vec![("A", "aaa"), ("B", "bbb"), ("C", "ccc")]);
        assert_eq!(app.compose_state.as_ref().unwrap().cursor, 0);

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.compose_state.as_ref().unwrap().cursor, 1);

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.compose_state.as_ref().unwrap().cursor, 2);

        // Can't go past last
        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.compose_state.as_ref().unwrap().cursor, 2);

        app.handle_key_event(key_event(KeyCode::Char('k')));
        assert_eq!(app.compose_state.as_ref().unwrap().cursor, 1);
    }

    #[test]
    fn space_toggles_selection_and_appends() {
        let mut app = app_with_library(vec![("A", "aaa"), ("B", "bbb"), ("C", "ccc")]);

        // Select first
        app.handle_key_event(key_event(KeyCode::Char(' ')));
        assert_eq!(app.compose_state.as_ref().unwrap().selected, vec![0]);

        // Select third (skip second)
        app.handle_key_event(key_event(KeyCode::Char('j')));
        app.handle_key_event(key_event(KeyCode::Char('j')));
        app.handle_key_event(key_event(KeyCode::Char(' ')));
        assert_eq!(app.compose_state.as_ref().unwrap().selected, vec![0, 2]);

        // Toggle first off
        app.handle_key_event(key_event(KeyCode::Char('k')));
        app.handle_key_event(key_event(KeyCode::Char('k')));
        app.handle_key_event(key_event(KeyCode::Char(' ')));
        assert_eq!(app.compose_state.as_ref().unwrap().selected, vec![2]);
    }

    #[test]
    fn selection_order_determines_compose_order() {
        let mut app = app_with_library(vec![("A", "aaa"), ("B", "bbb"), ("C", "ccc")]);

        // Select C first, then A
        app.handle_key_event(key_event(KeyCode::Char('j')));
        app.handle_key_event(key_event(KeyCode::Char('j')));
        app.handle_key_event(key_event(KeyCode::Char(' '))); // C
        app.handle_key_event(key_event(KeyCode::Char('k')));
        app.handle_key_event(key_event(KeyCode::Char('k')));
        app.handle_key_event(key_event(KeyCode::Char(' '))); // A

        // Composed output should be C then A (selection order)
        assert_eq!(app.composed_text(), "ccc\n\naaa");
    }

    #[test]
    fn esc_returns_to_files_screen() {
        let mut app = app_with_library(vec![("A", "aaa")]);
        app.handle_key_event(key_event(KeyCode::Esc));
        assert_eq!(app.screen, Screen::Files);
    }

    #[test]
    fn q_quits_app() {
        let mut app = app_with_library(vec![("A", "aaa")]);
        app.handle_key_event(key_event(KeyCode::Char('q')));
        assert!(app.exit);
    }

    #[test]
    fn tab_switches_pane_focus() {
        let mut app = app_with_library(vec![("A", "aaa")]);
        assert_eq!(
            app.compose_state.as_ref().unwrap().active_pane,
            ComposePane::List
        );

        app.handle_key_event(key_event(KeyCode::Tab));
        assert_eq!(
            app.compose_state.as_ref().unwrap().active_pane,
            ComposePane::Preview
        );

        app.handle_key_event(key_event(KeyCode::Tab));
        assert_eq!(
            app.compose_state.as_ref().unwrap().active_pane,
            ComposePane::List
        );
    }

    #[test]
    fn jk_scrolls_preview_when_focused() {
        let mut app = app_with_library(vec![("A", "line1\nline2\nline3\nline4\nline5")]);
        // Select snippet
        app.handle_key_event(key_event(KeyCode::Char(' ')));
        // Switch to preview
        app.handle_key_event(key_event(KeyCode::Tab));

        app.handle_key_event(key_event(KeyCode::Char('j')));
        // Preview scroll should not panic (viewport may be 0 in tests)
    }

    #[test]
    fn w_enters_export_path_mode() {
        let mut app = app_with_library(vec![("A", "aaa")]);
        app.handle_key_event(key_event(KeyCode::Char(' ')));
        app.handle_key_event(key_event(KeyCode::Char('w')));

        assert_eq!(app.mode, Mode::ExportPath);
    }

    #[test]
    fn w_shows_error_when_nothing_selected() {
        let mut app = app_with_library(vec![("A", "aaa")]);
        app.handle_key_event(key_event(KeyCode::Char('w')));

        assert_eq!(app.mode, Mode::Normal);
        assert!(
            app.status_message
                .as_deref()
                .unwrap()
                .contains("No snippets selected")
        );
    }

    #[test]
    fn export_writes_file_in_selection_order() {
        let tmp = TempDir::new().unwrap();
        let output_path = tmp.path().join("output.md");

        let mut app = app_with_library(vec![("A", "aaa"), ("B", "bbb")]);
        // Select B then A
        app.handle_key_event(key_event(KeyCode::Char('j')));
        app.handle_key_event(key_event(KeyCode::Char(' '))); // B
        app.handle_key_event(key_event(KeyCode::Char('k')));
        app.handle_key_event(key_event(KeyCode::Char(' '))); // A

        app.handle_key_event(key_event(KeyCode::Char('w')));
        for c in output_path.display().to_string().chars() {
            app.handle_key_event(key_event(KeyCode::Char(c)));
        }
        app.handle_key_event(key_event(KeyCode::Enter));

        assert_eq!(app.mode, Mode::Normal);
        let content = fs::read_to_string(&output_path).unwrap();
        assert_eq!(content, "bbb\n\naaa");
    }

    #[test]
    fn export_refuses_existing_file() {
        let tmp = TempDir::new().unwrap();
        let output_path = tmp.path().join("existing.md");
        fs::write(&output_path, "existing content").unwrap();

        let mut app = app_with_library(vec![("A", "aaa")]);
        app.handle_key_event(key_event(KeyCode::Char(' ')));
        app.handle_key_event(key_event(KeyCode::Char('w')));

        for c in output_path.display().to_string().chars() {
            app.handle_key_event(key_event(KeyCode::Char(c)));
        }
        app.handle_key_event(key_event(KeyCode::Enter));

        assert!(
            app.status_message
                .as_deref()
                .unwrap()
                .contains("already exists")
        );
        assert_eq!(
            fs::read_to_string(&output_path).unwrap(),
            "existing content"
        );
    }

    #[test]
    fn export_refuses_missing_parent() {
        let mut app = app_with_library(vec![("A", "aaa")]);
        app.handle_key_event(key_event(KeyCode::Char(' ')));
        app.handle_key_event(key_event(KeyCode::Char('w')));

        for c in "/nonexistent/dir/output.md".chars() {
            app.handle_key_event(key_event(KeyCode::Char(c)));
        }
        app.handle_key_event(key_event(KeyCode::Enter));

        assert!(
            app.status_message
                .as_deref()
                .unwrap()
                .contains("Parent directory does not exist")
        );
    }

    #[test]
    fn esc_cancels_export_path() {
        let mut app = app_with_library(vec![("A", "aaa")]);
        app.handle_key_event(key_event(KeyCode::Char(' ')));
        app.handle_key_event(key_event(KeyCode::Char('w')));
        assert_eq!(app.mode, Mode::ExportPath);

        app.handle_key_event(key_event(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn compose_state_preserved_across_tab_switch() {
        let mut app = app_with_library(vec![("A", "aaa"), ("B", "bbb")]);
        app.handle_key_event(key_event(KeyCode::Char(' ')));

        app.handle_key_event(key_event(KeyCode::Char('1')));
        assert_eq!(app.screen, Screen::Files);

        app.handle_key_event(key_event(KeyCode::Char('3')));
        assert_eq!(app.screen, Screen::Compose);

        assert_eq!(app.compose_state.as_ref().unwrap().selected, vec![0]);
    }

    #[test]
    fn composed_text_follows_selection_order() {
        let mut app = app_with_library(vec![("A", "aaa"), ("B", "bbb"), ("C", "ccc")]);
        // Select C then A
        app.handle_key_event(key_event(KeyCode::Char('j')));
        app.handle_key_event(key_event(KeyCode::Char('j')));
        app.handle_key_event(key_event(KeyCode::Char(' '))); // C
        app.handle_key_event(key_event(KeyCode::Char('k')));
        app.handle_key_event(key_event(KeyCode::Char('k')));
        app.handle_key_event(key_event(KeyCode::Char(' '))); // A

        assert_eq!(app.composed_text(), "ccc\n\naaa");
    }

    #[test]
    fn compose_dual_pane_renders_without_panic() {
        let mut app = app_with_library(vec![("A", "aaa"), ("B", "bbb")]);
        app.handle_key_event(key_event(KeyCode::Char(' ')));
        render_once(&mut app);
    }

    #[test]
    fn esc_from_preview_pane_returns_to_files() {
        let mut app = app_with_library(vec![("A", "aaa")]);
        app.handle_key_event(key_event(KeyCode::Tab)); // focus preview
        app.handle_key_event(key_event(KeyCode::Esc));
        assert_eq!(app.screen, Screen::Files);
    }
}
