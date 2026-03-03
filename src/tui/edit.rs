use std::cell::Cell;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use tui_textarea::TextArea;

use super::app::App;
use super::app::EditState;
use super::app::Mode;
use super::app::Screen;

impl App {
    pub(crate) fn draw_edit_pane(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        // Use take()/put-back pattern for the mutable borrow
        let mut edit = match self.edit_state.take() {
            Some(e) => e,
            None => return,
        };

        // Show full path for settings files, just filename for CLAUDE.md
        let display_name = if self.screen == Screen::Settings {
            edit.file_path.display().to_string()
        } else {
            edit.file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| edit.file_path.display().to_string())
        };

        let dirty_marker = if edit.is_dirty() { " [*]" } else { "" };
        let title = format!("Edit: {display_name}{dirty_marker}");

        edit.textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(self.theme.input_border)
                .title(title),
        );

        frame.render_widget(&edit.textarea, area);

        // Put back
        self.edit_state = Some(edit);
    }

    pub(crate) fn enter_edit_mode(&mut self) {
        let selected = self.tree_state.selected();
        if selected.len() < 2 {
            return;
        }
        let path_str = match selected.last() {
            Some(s) => s.clone(),
            None => return,
        };
        let path = PathBuf::from(&path_str);
        self.enter_edit_mode_for(&path);
    }

    /// Maximum file size (in bytes) allowed for editing. Files larger than this
    /// are rejected to prevent excessive memory usage.
    const MAX_EDIT_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

    /// Enters edit mode for a specific file path. Extracted for testability.
    pub fn enter_edit_mode_for(&mut self, path: &Path) {
        // Guard against very large files to prevent OOM
        match fs::metadata(path) {
            Ok(meta) if meta.len() > Self::MAX_EDIT_FILE_SIZE => {
                let size_mb = meta.len() as f64 / (1024.0 * 1024.0);
                self.status_message = Some(format!(
                    "File too large to edit ({size_mb:.1} MB, max 10 MB)"
                ));
                return;
            }
            Err(err) => {
                self.status_message = Some(format!("Cannot open for editing: {err}"));
                return;
            }
            _ => {}
        }

        let raw = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                self.status_message = Some(format!("Cannot open for editing: {err}"));
                return;
            }
        };

        let had_trailing_newline = raw.ends_with('\n');
        let text = if had_trailing_newline {
            raw.strip_suffix('\n').unwrap_or(&raw).to_string()
        } else {
            raw
        };

        let lines: Vec<String> = text.lines().map(String::from).collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };

        let mut textarea = TextArea::new(lines);
        textarea.set_tab_length(4);
        textarea.set_cursor_line_style(self.theme.edit_cursor_line);

        self.edit_state = Some(EditState {
            textarea,
            file_path: path.to_path_buf(),
            original_text: text,
            had_trailing_newline,
            discard_confirmed: false,
            dirty_cache: Cell::new(Some(false)),
        });
        self.mode = Mode::Edit;
    }

    pub(crate) fn handle_edit_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('s') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_edit();
            }
            KeyCode::Esc => {
                self.exit_edit_mode();
            }
            _ => {
                // Forward all other keys to the textarea
                if let Some(edit) = &mut self.edit_state {
                    edit.textarea.input(key_event);
                    edit.invalidate_dirty_cache();
                    // Any non-Esc key resets the discard confirmation
                    edit.discard_confirmed = false;
                }
            }
        }
    }

    fn save_edit(&mut self) {
        // If editing a library snippet, save back to library
        if self.editing_snippet_index.is_some() {
            self.save_snippet_edit();
            return;
        }

        let Some(edit) = &self.edit_state else {
            return;
        };
        let path = edit.file_path.clone();
        self.save_edit_to(&path);
    }

    /// Saves the current edit to a specific path. Extracted for testability.
    pub fn save_edit_to(&mut self, path: &Path) {
        let Some(edit) = &mut self.edit_state else {
            return;
        };

        let joined = edit.textarea.lines().join("\n");
        let write_content = if edit.had_trailing_newline {
            format!("{joined}\n")
        } else {
            joined.clone()
        };

        // Atomic write: write to temp file in the same directory, then rename.
        let parent = path.parent().unwrap_or(Path::new("."));
        let result = tempfile::NamedTempFile::new_in(parent).and_then(|mut tmp| {
            tmp.write_all(write_content.as_bytes())?;
            tmp.flush()?;
            tmp.persist(path).map_err(|e| e.error)?;
            Ok(())
        });

        match result {
            Ok(()) => {
                // Update original_text so the dirty flag clears
                edit.original_text = joined;
                edit.dirty_cache.set(Some(false));
                self.status_message = Some("Saved.".to_string());
            }
            Err(err) => {
                self.status_message = Some(format!("Save failed: {err}"));
            }
        }
    }

    fn exit_edit_mode(&mut self) {
        let Some(edit) = &mut self.edit_state else {
            self.finalize_exit_edit();
            return;
        };

        if edit.is_dirty() && !edit.discard_confirmed {
            edit.discard_confirmed = true;
            self.status_message =
                Some("You have unsaved changes. Press Esc again to discard.".to_string());
            return;
        }

        self.finalize_exit_edit();
    }

    pub(crate) fn finalize_exit_edit(&mut self) {
        // Reload content into the read-only viewer if on Files screen
        if self.screen == Screen::Files
            && let Some(edit) = &self.edit_state
        {
            self.load_file_content(&edit.file_path.clone());
        }

        // If on Settings screen, refresh the formatted view
        if self.screen == Screen::Settings {
            self.refresh_settings();
        }

        self.edit_state = None;
        self.editing_snippet_index = None;
        self.mode = Mode::Normal;
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyCode;
    use ratatui::crossterm::event::KeyEvent;
    use ratatui::crossterm::event::KeyEventKind;
    use ratatui::crossterm::event::KeyEventState;
    use ratatui::crossterm::event::KeyModifiers;
    use tui_textarea::TextArea;

    use tempfile::TempDir;

    use crate::config::Config;
    use crate::model::SourceRoot;
    use crate::tui::app::App;
    use crate::tui::app::EditState;
    use crate::tui::app::Mode;
    use crate::tui::app::Pane;
    use crate::tui::app::test_helpers::key_event;

    #[test]
    fn edit_state_starts_as_none() {
        let app = App::new(vec![], &Config::default());
        assert!(app.edit_state.is_none());
    }

    #[test]
    fn ctrl_c_exits_from_edit_mode() {
        let mut app = App::new(vec![], &Config::default());
        app.mode = Mode::Edit;
        app.handle_key_event(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        });
        assert!(app.exit, "Ctrl-C should exit from edit mode");
    }

    #[test]
    fn edit_state_is_dirty_detects_changes() {
        let original = "line 1\nline 2".to_string();
        let textarea = TextArea::from(["line 1", "line 2"]);
        let state = EditState {
            textarea,
            file_path: PathBuf::from("/test/file.md"),
            original_text: original,
            had_trailing_newline: false,
            discard_confirmed: false,
            dirty_cache: Cell::new(None),
        };
        assert!(!state.is_dirty(), "Unmodified textarea should not be dirty");

        let modified_textarea = TextArea::from(["line 1", "line 2 modified"]);
        let state2 = EditState {
            textarea: modified_textarea,
            file_path: PathBuf::from("/test/file.md"),
            original_text: "line 1\nline 2".to_string(),
            had_trailing_newline: false,
            discard_confirmed: false,
            dirty_cache: Cell::new(None),
        };
        assert!(state2.is_dirty(), "Modified textarea should be dirty");
    }

    #[test]
    fn e_in_content_pane_enters_edit_mode() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Hello world").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file],
        }];
        let mut app = App::new(roots, &Config::default());
        app.active_pane = Pane::Content;

        app.handle_key_event(key_event(KeyCode::Char('e')));

        assert_eq!(app.mode, Mode::Edit);
        assert!(app.edit_state.is_some());
        let edit = app.edit_state.as_ref().unwrap();
        assert_eq!(edit.textarea.lines().join("\n"), "Hello world");
        assert!(!edit.is_dirty());
    }

    #[test]
    fn edit_mode_renders_without_panic() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Line 1\nLine 2\nLine 3").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file],
        }];
        let mut app = App::new(roots, &Config::default());
        app.active_pane = Pane::Content;
        app.handle_key_event(key_event(KeyCode::Char('e')));
        assert_eq!(app.mode, Mode::Edit);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        // If we get here without panic, the test passes
    }

    #[test]
    fn e_in_file_list_does_not_enter_edit() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Hello").unwrap();

        let roots = vec![SourceRoot {
            path: tmp.path().to_path_buf(),
            files: vec![file],
        }];
        let mut app = App::new(roots, &Config::default());
        app.active_pane = Pane::FileList;

        app.handle_key_event(key_event(KeyCode::Char('e')));

        assert_eq!(app.mode, Mode::Normal);
        assert!(app.edit_state.is_none());
    }

    #[test]
    fn typing_in_edit_mode_modifies_textarea() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Hello").unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&file);
        assert_eq!(app.mode, Mode::Edit);

        // Type a character
        app.handle_key_event(key_event(KeyCode::Char('X')));

        let edit = app.edit_state.as_ref().unwrap();
        let content = edit.textarea.lines().join("\n");
        assert!(content.contains('X'), "Typed char should appear: {content}");
        assert!(edit.is_dirty());
    }

    #[test]
    fn ctrl_s_saves_file_to_disk() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "original").unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&file);

        // Type something
        app.handle_key_event(key_event(KeyCode::Char('!')));

        // Save with Ctrl+S
        app.handle_key_event(KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        });

        let saved = fs::read_to_string(&file).unwrap();
        assert!(
            saved.contains('!'),
            "File should contain typed char: {saved}"
        );
        assert!(app.status_message.as_deref().unwrap().contains("Saved"));

        // Should still be in edit mode after save
        assert_eq!(app.mode, Mode::Edit);
        // But no longer dirty
        assert!(
            !app.edit_state.as_ref().unwrap().is_dirty(),
            "After save, should not be dirty"
        );
    }

    #[test]
    fn esc_on_clean_edit_returns_to_normal() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "clean").unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&file);

        // Esc with no changes should exit edit mode
        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.mode, Mode::Normal);
        assert!(app.edit_state.is_none());
    }

    #[test]
    fn esc_on_dirty_edit_warns_does_not_exit() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "original").unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&file);
        app.handle_key_event(key_event(KeyCode::Char('X')));

        // First Esc: warns but doesn't exit
        app.handle_key_event(key_event(KeyCode::Esc));

        assert_eq!(app.mode, Mode::Edit, "Should still be in edit mode");
        assert!(
            app.status_message.as_deref().unwrap().contains("unsaved"),
            "Should show unsaved warning"
        );
    }

    #[test]
    fn double_esc_discards_unsaved_changes() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "original").unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&file);
        app.handle_key_event(key_event(KeyCode::Char('X')));

        // First Esc: warns
        app.handle_key_event(key_event(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Edit);

        // Second Esc: discards
        app.handle_key_event(key_event(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.edit_state.is_none());

        // File should be unchanged
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "original");
    }

    #[test]
    fn typing_after_first_esc_resets_discard_flag() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "original").unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&file);
        app.handle_key_event(key_event(KeyCode::Char('X')));

        // First Esc: warns
        app.handle_key_event(key_event(KeyCode::Esc));
        assert!(app.edit_state.as_ref().unwrap().discard_confirmed);

        // Type something: resets discard flag
        app.handle_key_event(key_event(KeyCode::Char('Y')));
        assert!(!app.edit_state.as_ref().unwrap().discard_confirmed);

        // Now Esc again should warn, not discard
        app.handle_key_event(key_event(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Edit, "Should still be in edit mode");
    }

    #[test]
    fn q_in_edit_mode_types_q_not_exit() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        fs::write(&file, "Hello").unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&file);

        app.handle_key_event(key_event(KeyCode::Char('q')));

        assert!(!app.exit, "q should not exit in edit mode");
        assert_eq!(app.mode, Mode::Edit);
        let edit = app.edit_state.as_ref().unwrap();
        let content = edit.textarea.lines().join("\n");
        assert!(content.contains('q'), "q should be typed into editor");
    }

    #[test]
    fn enter_edit_mode_for_nonexistent_file_stays_normal() {
        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(Path::new("/nonexistent/CLAUDE.md"));

        assert_eq!(app.mode, Mode::Normal, "Should stay in Normal mode");
        assert!(app.edit_state.is_none(), "No edit state should be created");
        assert!(
            app.status_message
                .as_deref()
                .unwrap()
                .contains("Cannot open"),
            "Should show error message, got: {:?}",
            app.status_message
        );
    }

    #[test]
    fn trailing_newline_preserved_after_edit_save_cycle() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        let original_content = "Line 1\nLine 2\n";
        fs::write(&file, original_content).unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&file);
        assert_eq!(app.mode, Mode::Edit);

        // Save without making changes
        app.handle_key_event(KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        });

        let saved = fs::read_to_string(&file).unwrap();
        assert_eq!(
            saved, original_content,
            "File should be byte-for-byte identical after no-op save"
        );
    }

    #[test]
    fn no_trailing_newline_preserved_after_edit_save_cycle() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("CLAUDE.md");
        let original_content = "Line 1\nLine 2";
        fs::write(&file, original_content).unwrap();

        let mut app = App::new(vec![], &Config::default());
        app.enter_edit_mode_for(&file);

        // Save without making changes
        app.handle_key_event(KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        });

        let saved = fs::read_to_string(&file).unwrap();
        assert_eq!(
            saved, original_content,
            "File without trailing newline should stay without one"
        );
    }
}
