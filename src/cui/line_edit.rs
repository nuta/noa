use crate::{KeyCode, KeyEvent, KeyModifiers};
use noa_buffer::{Point, Range, Rope};

pub struct LineEdit {
    rope: Rope,
    cursor: usize,
    scroll: usize,
}

impl LineEdit {
    pub fn new() -> LineEdit {
        LineEdit {
            rope: Rope::new(),
            cursor: 0,
            scroll: 0,
        }
    }

    pub fn from_str(text: &str) -> LineEdit {
        let mut le = LineEdit::new();
        le.insert(text);
        le
    }

    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rope.line_len(0)
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn text(&self) -> String {
        self.rope.text()
    }

    pub fn clear(&mut self) {
        self.rope.clear();
        self.cursor = 0;
    }

    pub fn cursor_display_pos(&self) -> usize {
        self.cursor - self.scroll
    }

    fn cursor_as_pos(&self) -> Point {
        Point::new(0, self.cursor)
    }

    pub fn set_text(&mut self, text: &str) {
        self.clear();
        self.insert(text);
    }

    pub fn insert(&mut self, s: &str) {
        self.rope.insert(self.cursor_as_pos(), s);
        self.cursor += s.chars().count();
    }

    pub fn insert_char(&mut self, c: char) {
        self.rope.insert_char(self.cursor_as_pos(), c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }

        self.rope
            .remove(&Range::new(0, self.cursor - 1, 0, self.cursor));
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        if self.cursor == self.len() {
            return;
        }

        self.rope
            .remove(&Range::new(0, self.cursor, 0, self.cursor + 1));
    }

    pub fn relocate_scroll(&mut self, width: usize) {
        // Scroll Right.
        if self.cursor > self.scroll + width {
            self.scroll = self.cursor - width;
        }

        // Scroll Left.
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.len() {
            self.cursor += 1;
        }
    }

    pub fn move_to_end_of_line(&mut self) {
        self.cursor = self.len();
    }

    pub fn move_to_beginning_of_line(&mut self) {
        self.cursor = 0;
    }

    pub fn move_to_next_word(&mut self) {
        let new_pos = self.rope.next_word_end(&self.cursor_as_pos());
        self.cursor = new_pos.x;
    }

    pub fn move_to_prev_word(&mut self) {
        let new_pos = self.rope.prev_word_end(&self.cursor_as_pos());
        self.cursor = new_pos.x;
    }

    pub fn consume_key_event(&mut self, key: KeyEvent) -> bool {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        match (key.modifiers, key.code) {
            (NONE, KeyCode::Char(ch)) => {
                self.insert_char(ch);
            }
            (SHIFT, KeyCode::Char(ch)) => {
                self.insert_char(ch);
            }
            (NONE, KeyCode::Backspace) => {
                self.backspace();
            }
            (NONE, KeyCode::Delete) | (CTRL, KeyCode::Char('d')) => {
                self.delete();
            }
            (NONE, KeyCode::Left) => {
                self.move_left();
            }
            (NONE, KeyCode::Right) => {
                self.move_right();
            }
            (CTRL, KeyCode::Char('a')) => {
                self.move_to_beginning_of_line();
            }
            (CTRL, KeyCode::Char('e')) => {
                self.move_to_end_of_line();
            }
            (ALT, KeyCode::Char('f')) => {
                self.move_to_next_word();
            }
            (ALT, KeyCode::Char('b')) => {
                self.move_to_prev_word();
            }
            _ => {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relocate_scroll() {
        let mut le = LineEdit::new();
        le.insert("abcde");
        le.relocate_scroll(5);
        assert_eq!(le.scroll, 0);

        le.relocate_scroll(4);
        assert_eq!(le.scroll, 1);

        le.insert_char('f');
        le.relocate_scroll(4);
        assert_eq!(le.scroll, 2);

        le.move_to_beginning_of_line();
        le.relocate_scroll(4);
        assert_eq!(le.scroll, 0);
    }
}
