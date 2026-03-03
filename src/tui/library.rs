use std::path::Path;

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

use super::app::App;
use super::app::Mode;
use super::app::Screen;

impl App {
    /// Switches to the Library screen, loading the library from disk if needed.
    pub(crate) fn enter_library_screen(&mut self) {
        match crate::library::library_path() {
            Some(path) => self.enter_library_screen_from(&path),
            None => {
                self.status_message = Some("Cannot determine library path.".to_string());
            }
        }
    }

    /// Switches to the Library screen using a specific library path. Extracted
    /// for testability.
    pub fn enter_library_screen_from(&mut self, path: &Path) {
        match crate::library::load_library(path) {
            Ok(lib) => {
                self.library = Some(lib);
                self.library_selected = 0;
                self.screen = Screen::Library;
                self.mode = Mode::Normal;
            }
            Err(err) => {
                self.status_message = Some(format!("Failed to load library: {err}"));
            }
        }
    }

    /// Draws the full Library screen (snippet list top 40%, preview bottom 60%).
    pub(crate) fn draw_library_screen(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let border_style = self.theme.active_border;

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

        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        // Left pane: snippet list
        let list_title = format!("Library ({} snippets)", lib.snippets.len());
        let list_lines: Vec<Line> = lib
            .snippets
            .iter()
            .enumerate()
            .map(|(i, snippet)| {
                let style = if i == self.library_selected {
                    self.theme.highlight
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
        frame.render_widget(list_widget, panes[0]);

        // Right pane: snippet content
        let preview_content = lib
            .snippets
            .get(self.library_selected)
            .map(|s| s.content.as_str())
            .unwrap_or("");
        let preview_title = lib
            .snippets
            .get(self.library_selected)
            .map(|s| s.title.as_str())
            .unwrap_or("Content");
        let preview_widget = Paragraph::new(preview_content).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(preview_title),
        );
        frame.render_widget(preview_widget, panes[1]);
    }

    /// Handles Normal-mode keys on the Library screen.
    pub(crate) fn handle_library_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.screen = Screen::Files;
            }
            KeyCode::Char('q') => {
                self.exit = true;
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
                    self.title_cursor = self.title_input.len();
                    self.mode = Mode::RenameInput;
                }
            }
            _ => {}
        }
    }

    /// Handles RenameInput-mode keys on the Library screen.
    pub(crate) fn handle_library_rename_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.title_input.clear();
                self.title_cursor = 0;
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                self.rename_library_snippet();
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

    fn rename_library_snippet(&mut self) {
        match crate::library::library_path() {
            Some(path) => self.rename_library_snippet_from(&path),
            None => {
                self.status_message = Some("Cannot determine library path.".to_string());
                self.title_input.clear();
                self.title_cursor = 0;
                self.mode = Mode::Normal;
            }
        }
    }

    /// Renames a library snippet. Extracted for testability.
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
                self.compose_state = None;
                self.status_message = Some("Snippet renamed.".to_string());
            }
            Err(err) => {
                self.status_message = Some(format!("Rename failed: {err}"));
            }
        }

        self.title_input.clear();
        self.title_cursor = 0;
        self.mode = Mode::Normal;
    }

    fn delete_library_snippet(&mut self) {
        match crate::library::library_path() {
            Some(path) => self.delete_library_snippet_from(&path),
            None => {
                self.status_message = Some("Cannot determine library path.".to_string());
            }
        }
    }

    /// Deletes a library snippet at a specific path. Extracted for testability.
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
                self.compose_state = None;
                self.status_message = Some("Snippet deleted.".to_string());
            }
            Err(err) => {
                self.status_message = Some(format!("Delete failed: {err}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;

    use tempfile::TempDir;

    use crate::config::Config;
    use crate::tui::app::App;
    use crate::tui::app::Mode;
    use crate::tui::app::Screen;
    use crate::tui::app::test_helpers::key_event;

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
    fn enter_library_screen_loads_snippets() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["Snippet A"]);

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);

        assert_eq!(app.screen, Screen::Library);
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.library_selected, 0);
        assert!(app.library.is_some());
        assert_eq!(app.library.as_ref().unwrap().snippets.len(), 1);
    }

    #[test]
    fn esc_on_library_screen_returns_to_files() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);
        assert_eq!(app.screen, Screen::Library);

        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.screen, Screen::Files);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn q_on_library_screen_exits_app() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);

        app.handle_key_event(key_event(KeyCode::Char('q')));

        assert!(app.exit, "q should exit the app from Library screen");
    }

    #[test]
    fn jk_on_library_screen_navigates() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["A", "B", "C"]);

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);
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
    fn d_on_library_screen_deletes_snippet() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["A", "B", "C"]);

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);

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
    fn delete_last_snippet_adjusts_selected() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["A", "B"]);

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);

        // Select last item and delete
        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.library_selected, 1);

        app.delete_library_snippet_from(&lib_path);

        assert_eq!(app.library.as_ref().unwrap().snippets.len(), 1);
        assert_eq!(app.library_selected, 0, "Adjusted to last valid index");
    }

    #[test]
    fn delete_on_empty_library_is_noop() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);

        assert!(app.library.as_ref().unwrap().snippets.is_empty());

        app.delete_library_snippet_from(&lib_path);

        assert!(app.library.as_ref().unwrap().snippets.is_empty());
    }

    #[test]
    fn library_screen_loads_from_disk() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["X", "Y"]);

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);

        let lib = app.library.as_ref().unwrap();
        assert_eq!(lib.snippets.len(), 2);
        assert_eq!(lib.snippets[0].title, "X");
        assert_eq!(lib.snippets[1].title, "Y");
    }

    #[test]
    fn r_on_library_screen_enters_rename_with_current_title() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["My Snippet"]);

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);

        app.handle_key_event(key_event(KeyCode::Char('r')));

        assert_eq!(app.mode, Mode::RenameInput);
        assert_eq!(app.title_input, "My Snippet");
    }

    #[test]
    fn rename_esc_returns_to_normal_on_library_screen() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);
        app.mode = Mode::RenameInput;
        app.title_input = "partial edit".to_string();

        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.screen, Screen::Library);
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.title_input.is_empty());
    }

    #[test]
    fn rename_saves_new_title() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");
        library_with_snippets(&lib_path, &["Old Title"]);

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);
        app.mode = Mode::RenameInput;
        app.title_input = "New Title".to_string();

        app.rename_library_snippet_from(&lib_path);

        assert_eq!(app.screen, Screen::Library);
        assert_eq!(app.mode, Mode::Normal);
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

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);
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
    fn r_on_empty_library_does_nothing() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);

        app.handle_key_event(key_event(KeyCode::Char('r')));

        assert_eq!(app.screen, Screen::Library);
        assert_eq!(app.mode, Mode::Normal, "Stays in Normal on empty lib");
    }

    #[test]
    fn number_keys_switch_screens_from_library() {
        let tmp = TempDir::new().unwrap();
        let lib_path = tmp.path().join("library.toml");

        let mut app = App::new(vec![], &Config::default());
        app.enter_library_screen_from(&lib_path);
        assert_eq!(app.screen, Screen::Library);

        app.handle_key_event(key_event(KeyCode::Char('1')));
        assert_eq!(app.screen, Screen::Files);
    }
}
