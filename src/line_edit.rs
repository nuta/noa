use crate::rope::{Point, Range, Rope};

pub struct LineEdit {
    rope: Rope,
    cursor: usize,
}

impl LineEdit {
    pub fn new() -> LineEdit {
        LineEdit {
            rope: Rope::new(),
            cursor: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.rope.line_len(0)
    }

    pub fn text(&self) -> String {
        self.rope.text()
    }

    pub fn clear(&mut self) {
        self.rope.clear();
        self.cursor = 0;
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn cursor_as_pos(&self) -> Point {
        Point::new(0, self.cursor)
    }

    pub fn insert(&mut self, s: &str) {
        self.rope.insert(&self.cursor_as_pos(), s);
        self.cursor += s.chars().count();
    }

    pub fn insert_char(&mut self, c: char) {
        self.rope.insert_char(&self.cursor_as_pos(), c);
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
}
