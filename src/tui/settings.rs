use std::path::Path;
use std::path::PathBuf;

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
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
use crate::settings::SettingsCollection;
use crate::settings::SettingsFile;
use crate::settings::format_settings_with_map;

impl App {
    pub(crate) fn draw_settings_screen(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        if self.mode == Mode::Edit {
            self.draw_edit_pane(frame, area);
            return;
        }

        self.settings_state.viewport_height = area.height.saturating_sub(2);

        let cursor_line = self.settings_state.cursor;
        let cursor_style = self.theme.highlight;

        // Only render visible lines (respecting collapsed sections).
        let lines: Vec<Line> = self
            .settings_state
            .lines
            .iter()
            .enumerate()
            .filter(|&(i, _)| self.settings_state.is_line_visible(i))
            .map(|(i, line_text)| {
                let style = if i == cursor_line {
                    cursor_style
                } else {
                    Style::default()
                };
                Line::from(line_text.as_str().to_string()).style(style)
            })
            .collect();

        let title = if self.settings_state.merged_view {
            "Settings — Effective"
        } else {
            "Settings"
        };
        let settings_widget = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title(title))
            .scroll((self.settings_state.scroll, 0));
        frame.render_widget(settings_widget, area);

        let visible_count = self.settings_state.visible_line_count();
        let mut scrollbar_state =
            ScrollbarState::new(visible_count).position(self.settings_state.scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }

    pub(crate) fn handle_settings_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('e') if !self.settings_state.merged_view => {
                self.enter_settings_edit_mode();
            }
            KeyCode::Char('e') => {
                self.status_message =
                    Some("Edit not available in merged view — press m to switch.".to_string());
            }
            KeyCode::Char('m') => {
                self.settings_state.merged_view = !self.settings_state.merged_view;
                self.rebuild_settings_display();
            }
            KeyCode::Char('q') => self.exit = true,
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings_state.cursor_down();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_state.cursor_up();
            }
            KeyCode::PageDown => {
                self.settings_state.cursor_page_down();
            }
            KeyCode::PageUp => {
                self.settings_state.cursor_page_up();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                let cursor = self.settings_state.cursor;
                if self.settings_state.is_foldable(cursor)
                    && !self.settings_state.collapsed.contains(&cursor)
                {
                    // On a foldable line: collapse it
                    self.settings_state.toggle_fold(cursor);
                } else if let Some(parent) = self.settings_state.parent_for(cursor) {
                    // On a child line: jump to parent
                    self.settings_state.cursor = parent;
                    self.settings_state.ensure_cursor_visible();
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let cursor = self.settings_state.cursor;
                if self.settings_state.collapsed.contains(&cursor) {
                    // On a collapsed line: expand it
                    self.settings_state.toggle_fold(cursor);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn switch_to_settings(&mut self) {
        let project = std::env::current_dir().unwrap_or_default();
        self.switch_to_settings_from(&project);
    }

    /// Switch to settings screen using an explicit project path (for testability).
    pub fn switch_to_settings_from(&mut self, project: &Path) {
        let collection = crate::settings::discover_settings_files(project);
        self.apply_settings_collection(collection);
        self.screen = Screen::Settings;
    }

    /// Switch to settings screen with a pre-built collection (for testability).
    #[cfg(test)]
    pub fn switch_to_settings_with(&mut self, collection: &SettingsCollection) {
        self.apply_settings_collection(collection.clone());
        self.screen = Screen::Settings;
    }

    fn apply_settings_collection(&mut self, collection: SettingsCollection) {
        self.settings_collection = Some(collection);
        self.settings_state.merged_view = false;
        self.rebuild_settings_display();
    }

    /// Rebuilds the settings display lines from the cached collection.
    ///
    /// Uses per-file formatting or merged formatting depending on
    /// `settings_state.merged_view`.
    fn rebuild_settings_display(&mut self) {
        let Some(collection) = &self.settings_collection else {
            return;
        };
        let (lines, line_map) = if self.settings_state.merged_view {
            let merged = crate::settings::merge_settings(collection);
            let synthetic = SettingsCollection {
                files: vec![SettingsFile {
                    label: "Effective".to_string(),
                    path: PathBuf::new(),
                    value: merged,
                }],
            };
            format_settings_with_map(&synthetic)
        } else {
            format_settings_with_map(collection)
        };
        self.settings_state.lines = lines;
        self.settings_state.line_map = line_map;
        self.settings_state.scroll = 0;
        self.settings_state.cursor = 0;
        self.settings_state.collapsed.clear();
    }

    /// Returns the file path of the settings file at the current cursor position.
    pub fn settings_file_at_cursor(&self) -> Option<&Path> {
        let file_idx = self
            .settings_state
            .line_map
            .get(self.settings_state.cursor)
            .copied()
            .flatten()?;
        let collection = self.settings_collection.as_ref()?;
        let file = collection.files.get(file_idx)?;
        Some(&file.path)
    }

    pub(crate) fn refresh_settings(&mut self) {
        let project = std::env::current_dir().unwrap_or_default();
        let collection = crate::settings::discover_settings_files(&project);
        self.apply_settings_collection(collection);
    }

    fn enter_settings_edit_mode(&mut self) {
        let path = match self.settings_file_at_cursor() {
            Some(p) => p.to_path_buf(),
            None => {
                self.status_message = Some("No settings file at cursor.".to_string());
                return;
            }
        };
        self.enter_edit_mode_for(&path);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use ratatui::crossterm::event::KeyCode;

    use crate::config::Config;
    use crate::tui::app::App;
    use crate::tui::app::Mode;
    use crate::tui::app::Screen;
    use crate::tui::app::test_helpers::key_event;

    #[test]
    fn app_starts_on_files_screen() {
        let app = App::new(vec![], &Config::default());
        assert_eq!(app.screen, Screen::Files);
    }

    #[test]
    fn pressing_2_switches_to_settings() {
        let mut app = App::new(vec![], &Config::default());
        let collection = crate::settings::SettingsCollection {
            files: vec![crate::settings::SettingsFile {
                label: "Test".to_string(),
                path: PathBuf::from("/test/settings.json"),
                value: serde_json::json!({"model": "opus"}),
            }],
        };
        app.switch_to_settings_with(&collection);
        assert_eq!(app.screen, Screen::Settings);
        assert!(!app.settings_state.lines.is_empty());
    }

    #[test]
    fn pressing_1_returns_to_files() {
        let mut app = App::new(vec![], &Config::default());
        app.screen = Screen::Settings;
        app.handle_key_event(key_event(KeyCode::Char('1')));
        assert_eq!(app.screen, Screen::Files);
    }

    #[test]
    fn pressing_2_in_title_input_types_char_not_switch() {
        let mut app = App::new(vec![], &Config::default());
        app.mode = Mode::TitleInput;
        app.handle_key_event(key_event(KeyCode::Char('2')));
        assert_eq!(app.screen, Screen::Files, "Should NOT switch screen");
        assert_eq!(app.title_input, "2", "Should type '2' into input");
    }

    #[test]
    fn q_on_settings_exits() {
        let mut app = App::new(vec![], &Config::default());
        app.screen = Screen::Settings;
        app.handle_key_event(key_event(KeyCode::Char('q')));
        assert!(app.exit);
    }

    #[test]
    fn jk_on_settings_scrolls() {
        let mut app = App::new(vec![], &Config::default());
        app.screen = Screen::Settings;
        app.settings_state.lines = vec![
            "Line 0".to_string(),
            "Line 1".to_string(),
            "Line 2".to_string(),
            "Line 3".to_string(),
        ];
        app.settings_state.viewport_height = 10;

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.settings_state.cursor, 1);

        app.handle_key_event(key_event(KeyCode::Char('j')));
        assert_eq!(app.settings_state.cursor, 2);

        app.handle_key_event(key_event(KeyCode::Char('k')));
        assert_eq!(app.settings_state.cursor, 1);
    }

    #[test]
    fn help_line_shows_edit_key_on_settings_screen() {
        let mut app = App::new(vec![], &Config::default());
        app.screen = Screen::Settings;
        let help = app.help_line();
        let help_text: String = help.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(
            help_text.contains("Edit"),
            "Help line should show Edit on settings screen: {help_text}"
        );
    }

    #[test]
    fn e_on_settings_screen_enters_edit_for_settings_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let settings_dir = tmp.path().join(".claude");
        fs::create_dir_all(&settings_dir).unwrap();
        let settings_file = settings_dir.join("settings.json");
        fs::write(&settings_file, r#"{"model":"opus"}"#).unwrap();

        let collection = crate::settings::SettingsCollection {
            files: vec![crate::settings::SettingsFile {
                label: "Test".to_string(),
                path: settings_file.clone(),
                value: serde_json::json!({"model": "opus"}),
            }],
        };

        let mut app = App::new(vec![], &Config::default());
        app.switch_to_settings_with(&collection);
        app.settings_state.cursor = 0; // on the header line of the file

        app.handle_key_event(key_event(KeyCode::Char('e')));

        assert_eq!(app.mode, Mode::Edit);
        assert!(app.edit_state.is_some());
        let edit = app.edit_state.as_ref().unwrap();
        assert_eq!(edit.file_path, settings_file);
    }

    #[test]
    fn exiting_settings_edit_refreshes_formatted_view() {
        let tmp = tempfile::TempDir::new().unwrap();
        let settings_dir = tmp.path().join(".claude");
        fs::create_dir_all(&settings_dir).unwrap();
        let settings_file = settings_dir.join("settings.json");
        fs::write(&settings_file, r#"{"model":"opus"}"#).unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&settings_file);
        app.screen = Screen::Settings;
        assert_eq!(app.mode, Mode::Edit);

        // Exit without changes
        app.handle_key_event(key_event(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.edit_state.is_none());
    }

    #[test]
    fn settings_file_at_cursor_resolves_path() {
        let mut app = App::new(vec![], &Config::default());
        let collection = crate::settings::SettingsCollection {
            files: vec![
                crate::settings::SettingsFile {
                    label: "Global".to_string(),
                    path: PathBuf::from("/home/.claude/settings.json"),
                    value: serde_json::json!({"model": "opus"}),
                },
                crate::settings::SettingsFile {
                    label: "Project".to_string(),
                    path: PathBuf::from("/proj/.claude/settings.json"),
                    value: serde_json::json!({"defaultMode": "plan"}),
                },
            ],
        };
        app.switch_to_settings_with(&collection);

        // Cursor at 0 should be the Global file header
        app.settings_state.cursor = 0;
        assert_eq!(
            app.settings_file_at_cursor(),
            Some(Path::new("/home/.claude/settings.json"))
        );

        // Find a line from the second file
        let second_file_line = app
            .settings_state
            .line_map
            .iter()
            .position(|m| *m == Some(1))
            .unwrap();
        app.settings_state.cursor = second_file_line;
        assert_eq!(
            app.settings_file_at_cursor(),
            Some(Path::new("/proj/.claude/settings.json"))
        );
    }

    #[test]
    fn e_on_blank_separator_in_settings_shows_error() {
        let mut app = App::new(vec![], &Config::default());
        let collection = crate::settings::SettingsCollection {
            files: vec![
                crate::settings::SettingsFile {
                    label: "Global".to_string(),
                    path: PathBuf::from("/home/.claude/settings.json"),
                    value: serde_json::json!({"model": "opus"}),
                },
                crate::settings::SettingsFile {
                    label: "Project".to_string(),
                    path: PathBuf::from("/proj/.claude/settings.json"),
                    value: serde_json::json!({"defaultMode": "plan"}),
                },
            ],
        };
        app.switch_to_settings_with(&collection);

        // Find the blank separator line (maps to None)
        let blank_idx = app
            .settings_state
            .line_map
            .iter()
            .position(|m| m.is_none())
            .unwrap();
        app.settings_state.cursor = blank_idx;

        app.handle_key_event(key_event(KeyCode::Char('e')));

        assert_eq!(app.mode, Mode::Normal, "Should stay in Normal mode");
        assert!(app.edit_state.is_none());
        assert!(
            app.status_message
                .as_deref()
                .unwrap()
                .contains("No settings file"),
            "Should show no-file message, got: {:?}",
            app.status_message
        );
    }

    fn two_file_settings_collection() -> crate::settings::SettingsCollection {
        crate::settings::SettingsCollection {
            files: vec![
                crate::settings::SettingsFile {
                    label: "Global".to_string(),
                    path: PathBuf::from("/home/.claude/settings.json"),
                    value: serde_json::json!({"model": "opus", "permissions": {"allow": ["Read"]}}),
                },
                crate::settings::SettingsFile {
                    label: "Project".to_string(),
                    path: PathBuf::from("/proj/.claude/settings.json"),
                    value: serde_json::json!({"model": "haiku", "permissions": {"allow": ["Write"]}}),
                },
            ],
        }
    }

    #[test]
    fn m_key_toggles_merged_view() {
        let mut app = App::new(vec![], &Config::default());
        app.switch_to_settings_with(&two_file_settings_collection());

        // Per-file view has two section headers
        let headers_before: Vec<_> = app
            .settings_state
            .lines
            .iter()
            .filter(|l| l.starts_with('\u{25be}'))
            .collect();
        assert_eq!(
            headers_before.len(),
            2,
            "Per-file view should have 2 headers"
        );

        // Toggle to merged
        app.handle_key_event(key_event(KeyCode::Char('m')));
        assert!(app.settings_state.merged_view);

        let headers_after: Vec<_> = app
            .settings_state
            .lines
            .iter()
            .filter(|l| l.starts_with('\u{25be}'))
            .collect();
        assert_eq!(headers_after.len(), 1, "Merged view should have 1 header");
        assert!(
            headers_after[0].contains("Effective"),
            "Header should say Effective, got: {}",
            headers_after[0]
        );
    }

    #[test]
    fn m_key_resets_cursor() {
        let mut app = App::new(vec![], &Config::default());
        app.switch_to_settings_with(&two_file_settings_collection());

        app.settings_state.cursor = 5;
        app.settings_state.scroll = 3;

        app.handle_key_event(key_event(KeyCode::Char('m')));

        assert_eq!(
            app.settings_state.cursor, 0,
            "Cursor should reset on toggle"
        );
        assert_eq!(
            app.settings_state.scroll, 0,
            "Scroll should reset on toggle"
        );
    }

    #[test]
    fn m_key_round_trip() {
        let mut app = App::new(vec![], &Config::default());
        app.switch_to_settings_with(&two_file_settings_collection());
        let lines_before = app.settings_state.lines.clone();

        // Toggle to merged, then back
        app.handle_key_event(key_event(KeyCode::Char('m')));
        app.handle_key_event(key_event(KeyCode::Char('m')));

        assert!(!app.settings_state.merged_view);
        assert_eq!(app.settings_state.lines, lines_before);
    }

    #[test]
    fn e_disabled_in_merged_view() {
        let mut app = App::new(vec![], &Config::default());
        app.switch_to_settings_with(&two_file_settings_collection());
        app.screen = Screen::Settings;

        // Toggle to merged
        app.handle_key_event(key_event(KeyCode::Char('m')));

        // Press e
        app.handle_key_event(key_event(KeyCode::Char('e')));

        assert_eq!(
            app.mode,
            Mode::Normal,
            "e should not enter edit in merged view"
        );
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or("")
                .contains("merged view"),
            "Should show merged view message, got: {:?}",
            app.status_message
        );
    }

    #[test]
    fn help_bar_shows_merge_key() {
        let mut app = App::new(vec![], &Config::default());
        app.screen = Screen::Settings;
        let help = app.help_line();
        let help_text: String = help.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(
            help_text.contains("Merge"),
            "Help bar should show Merge key in per-file view: {help_text}"
        );
        assert!(
            help_text.contains("Edit"),
            "Help bar should show Edit in per-file view: {help_text}"
        );
    }

    #[test]
    fn help_bar_in_merged_omits_edit() {
        let mut app = App::new(vec![], &Config::default());
        app.screen = Screen::Settings;
        app.settings_state.merged_view = true;
        let help = app.help_line();
        let help_text: String = help.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(
            help_text.contains("Per-file"),
            "Help bar should show Per-file in merged view: {help_text}"
        );
        assert!(
            !help_text.contains("Edit"),
            "Help bar should NOT show Edit in merged view: {help_text}"
        );
    }

    fn settings_app_with_lines(lines: Vec<&str>) -> App {
        let mut app = App::new(vec![], &Config::default());
        app.screen = Screen::Settings;
        app.settings_state.lines = lines.into_iter().map(String::from).collect();
        app.settings_state.line_map = vec![Some(0); app.settings_state.lines.len()];
        app
    }

    // --- Fold/unfold tests ---

    #[test]
    fn is_foldable_detects_lines_with_children() {
        let app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  Model: opus",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx rust-cargo",
        ]);
        assert!(app.settings_state.is_foldable(0), "Top header is foldable");
        assert!(!app.settings_state.is_foldable(1), "Leaf is not foldable");
        assert!(app.settings_state.is_foldable(2), "Sub-header is foldable");
        assert!(!app.settings_state.is_foldable(3), "Leaf is not foldable");
    }

    #[test]
    fn parent_for_returns_nearest_ancestor() {
        let app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
        ]);
        assert_eq!(app.settings_state.parent_for(0), None);
        assert_eq!(app.settings_state.parent_for(1), Some(0));
        assert_eq!(app.settings_state.parent_for(2), Some(1));
    }

    #[test]
    fn fold_top_level_hides_all_children() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  Model: opus",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
        ]);
        app.settings_state.toggle_fold(0);
        assert!(
            app.settings_state.is_line_visible(0),
            "Header stays visible"
        );
        assert!(!app.settings_state.is_line_visible(1));
        assert!(!app.settings_state.is_line_visible(2));
        assert!(!app.settings_state.is_line_visible(3));

        app.settings_state.toggle_fold(0);
        assert!(app.settings_state.is_line_visible(3), "All visible again");
    }

    #[test]
    fn fold_sub_section_hides_only_its_children() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  Model: opus",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
            "    github: gh",
            "  Thinking: true",
        ]);
        // Fold "  ▾ MCP Servers:"
        app.settings_state.toggle_fold(2);
        assert!(app.settings_state.is_line_visible(0));
        assert!(app.settings_state.is_line_visible(1), "Model still visible");
        assert!(app.settings_state.is_line_visible(2), "MCP header visible");
        assert!(!app.settings_state.is_line_visible(3), "rust-cargo hidden");
        assert!(!app.settings_state.is_line_visible(4), "github hidden");
        assert!(
            app.settings_state.is_line_visible(5),
            "Thinking still visible"
        );
    }

    #[test]
    fn cursor_down_skips_collapsed_lines() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  Model: opus",
            "  Thinking: true",
            "▾ Project (/other)",
            "  Model: sonnet",
        ]);
        app.settings_state.toggle_fold(0);
        app.settings_state.cursor = 0;

        app.settings_state.cursor_down();
        assert_eq!(app.settings_state.cursor, 3, "Should skip to next header");
    }

    #[test]
    fn cursor_up_skips_collapsed_lines() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  Model: opus",
            "  Thinking: true",
            "▾ Project (/other)",
            "  Model: sonnet",
        ]);
        app.settings_state.toggle_fold(0);
        app.settings_state.cursor = 3;

        app.settings_state.cursor_up();
        assert_eq!(app.settings_state.cursor, 0, "Should skip back to header");
    }

    #[test]
    fn cursor_skips_sub_section_collapsed_lines() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
            "    github: gh",
            "  Thinking: true",
        ]);
        app.settings_state.toggle_fold(1); // Fold MCP Servers
        app.settings_state.cursor = 1;

        app.settings_state.cursor_down();
        assert_eq!(app.settings_state.cursor, 4, "Should skip to Thinking");
    }

    #[test]
    fn left_arrow_on_foldable_collapses() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
        ]);
        app.settings_state.cursor = 1; // On "  ▾ MCP Servers:"

        app.handle_key_event(key_event(KeyCode::Left));
        assert!(
            app.settings_state.collapsed.contains(&1),
            "Sub-section should be collapsed"
        );
    }

    #[test]
    fn left_arrow_on_leaf_jumps_to_parent() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
        ]);
        app.settings_state.cursor = 2; // On "    rust-cargo: npx"

        app.handle_key_event(key_event(KeyCode::Left));
        assert_eq!(
            app.settings_state.cursor, 1,
            "Should jump to MCP Servers parent"
        );
    }

    #[test]
    fn left_on_collapsed_foldable_jumps_to_parent() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
        ]);
        app.settings_state.toggle_fold(1); // Already collapsed
        app.settings_state.cursor = 1;

        app.handle_key_event(key_event(KeyCode::Left));
        assert_eq!(
            app.settings_state.cursor, 0,
            "Should jump to top-level parent"
        );
    }

    #[test]
    fn right_arrow_on_collapsed_sub_section_expands() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
        ]);
        app.settings_state.toggle_fold(1);
        app.settings_state.cursor = 1;

        app.handle_key_event(key_event(KeyCode::Right));
        assert!(
            !app.settings_state.collapsed.contains(&1),
            "Sub-section should be expanded"
        );
    }

    #[test]
    fn right_arrow_on_expanded_is_noop() {
        let mut app = settings_app_with_lines(vec!["▾ Global (/path)", "  Model: opus"]);
        app.settings_state.cursor = 0;

        app.handle_key_event(key_event(KeyCode::Right));
        assert!(!app.settings_state.collapsed.contains(&0));
    }

    #[test]
    fn visible_line_count_respects_collapsed() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  Model: opus",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
            "    github: gh",
        ]);
        assert_eq!(app.settings_state.visible_line_count(), 5);

        app.settings_state.toggle_fold(2); // Fold MCP Servers
        assert_eq!(app.settings_state.visible_line_count(), 3);

        app.settings_state.toggle_fold(0); // Fold entire Global
        assert_eq!(app.settings_state.visible_line_count(), 1);
    }

    #[test]
    fn nested_fold_parent_hides_expanded_children() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
        ]);
        // Children of MCP Servers are expanded, but fold the parent
        app.settings_state.toggle_fold(0);
        assert!(!app.settings_state.is_line_visible(1), "MCP Servers hidden");
        assert!(!app.settings_state.is_line_visible(2), "rust-cargo hidden");
    }

    #[test]
    fn rebuild_settings_clears_collapsed_state() {
        let mut app = settings_app_with_lines(vec!["▾ Global (/path)", "  Model: opus"]);
        app.settings_state.collapsed.insert(0);

        app.settings_state.collapsed.clear();
        assert!(app.settings_state.collapsed.is_empty());
    }

    #[test]
    fn sub_header_indicator_toggles_on_fold() {
        let mut app = settings_app_with_lines(vec![
            "▾ Global (/path)",
            "  ▾ MCP Servers:",
            "    rust-cargo: npx",
        ]);
        assert!(app.settings_state.lines[1].contains('▾'));

        app.settings_state.toggle_fold(1);
        assert!(
            app.settings_state.lines[1].contains('▸'),
            "Should show collapsed indicator"
        );

        app.settings_state.toggle_fold(1);
        assert!(
            app.settings_state.lines[1].contains('▾'),
            "Should show expanded indicator again"
        );
    }

    #[test]
    fn top_header_indicator_toggles_on_fold() {
        let mut app = settings_app_with_lines(vec!["▾ Global (/path)", "  Model: opus"]);
        app.settings_state.toggle_fold(0);
        assert!(app.settings_state.lines[0].starts_with('▸'));

        app.settings_state.toggle_fold(0);
        assert!(app.settings_state.lines[0].starts_with('▾'));
    }
}
