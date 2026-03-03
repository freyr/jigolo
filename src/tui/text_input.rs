use ratatui::crossterm::event::KeyCode;

/// A single-line text input with cursor tracking.
///
/// Handles common editing keys (Backspace, Left, Right, Char insertion)
/// with cursor position invariants maintained internally.
#[derive(Debug, Default)]
pub struct TextInput {
    text: String,
    cursor: usize,
}

impl TextInput {
    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the current cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Clears text and resets cursor to 0.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Sets text content and places cursor at the end.
    pub fn set(&mut self, value: &str) {
        self.text = value.to_string();
        self.cursor = self.text.len();
    }

    /// Handles editing keys (Backspace, Left, Right, Char).
    ///
    /// Returns `true` if the key was consumed, `false` if ignored
    /// (caller should handle Enter, Esc, and other keys itself).
    pub fn handle_edit_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.text.remove(self.cursor);
                }
                true
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            KeyCode::Right => {
                if self.cursor < self.text.len() {
                    self.cursor += 1;
                }
                true
            }
            KeyCode::Char(c) => {
                self.text.insert(self.cursor, c);
                self.cursor += 1;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let input = TextInput::default();
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn char_inserts_at_cursor() {
        let mut input = TextInput::default();
        assert!(input.handle_edit_key(KeyCode::Char('a')));
        assert!(input.handle_edit_key(KeyCode::Char('b')));
        assert_eq!(input.text(), "ab");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn backspace_deletes_before_cursor() {
        let mut input = TextInput::default();
        input.set("abc");
        assert!(input.handle_edit_key(KeyCode::Backspace));
        assert_eq!(input.text(), "ab");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn backspace_at_start_is_noop() {
        let mut input = TextInput::default();
        input.set("abc");
        input.cursor = 0;
        assert!(input.handle_edit_key(KeyCode::Backspace));
        assert_eq!(input.text(), "abc");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn left_moves_cursor_back() {
        let mut input = TextInput::default();
        input.set("ab");
        assert_eq!(input.cursor(), 2);
        assert!(input.handle_edit_key(KeyCode::Left));
        assert_eq!(input.cursor(), 1);
    }

    #[test]
    fn left_at_start_stays() {
        let mut input = TextInput::default();
        assert!(input.handle_edit_key(KeyCode::Left));
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn right_moves_cursor_forward() {
        let mut input = TextInput::default();
        input.set("ab");
        input.cursor = 0;
        assert!(input.handle_edit_key(KeyCode::Right));
        assert_eq!(input.cursor(), 1);
    }

    #[test]
    fn right_at_end_stays() {
        let mut input = TextInput::default();
        input.set("ab");
        assert!(input.handle_edit_key(KeyCode::Right));
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn insert_in_middle() {
        let mut input = TextInput::default();
        input.set("ac");
        input.cursor = 1;
        input.handle_edit_key(KeyCode::Char('b'));
        assert_eq!(input.text(), "abc");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn clear_resets_everything() {
        let mut input = TextInput::default();
        input.set("hello");
        input.clear();
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn set_places_cursor_at_end() {
        let mut input = TextInput::default();
        input.set("hello");
        assert_eq!(input.text(), "hello");
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn unhandled_key_returns_false() {
        let mut input = TextInput::default();
        assert!(!input.handle_edit_key(KeyCode::Enter));
        assert!(!input.handle_edit_key(KeyCode::Esc));
        assert!(!input.handle_edit_key(KeyCode::Tab));
    }
}
