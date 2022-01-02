use noa_editorconfig::{EditorConfig, IndentStyle};

use crate::{
    cursor::{Cursor, CursorSet, Position},
    raw_buffer::RawBuffer,
};

pub struct Buffer {
    buf: RawBuffer,
    cursors: CursorSet,
    config: EditorConfig,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            buf: RawBuffer::new(),
            cursors: CursorSet::new(),
            config: EditorConfig::default(),
        }
    }

    pub fn from_text(text: &str) -> Buffer {
        Buffer {
            buf: RawBuffer::from_text(text),
            ..Default::default()
        }
    }

    pub fn len_chars(&self) -> usize {
        self.buf.len_chars()
    }

    pub fn num_lines(&self) -> usize {
        self.buf.num_lines()
    }

    pub fn line_len(&self, y: usize) -> usize {
        self.buf.line_len(y)
    }

    pub fn text(&self) -> String {
        self.buf.text()
    }

    pub fn set_cursors(&mut self, new_cursors: &[Cursor]) {
        self.cursors.set_cursors(new_cursors);
    }

    pub fn insert_char(&mut self, c: char) {
        self.insert_str(&c.to_string());
    }

    pub fn indent(&mut self) {
        // Compute indent sizes.
        let mut increase_lens = Vec::new();
        for c in &self.cursors {
            let pos = c.front();
            let (prev_indent_len, char_at_cursor) = if pos.y == 0 {
                (0, None)
            } else {
                let prev_indent_len = self.buf.line_indent_len(pos.y - 1);
                let pos_at_newline = Position::new(pos.y - 1, self.buf.line_len(pos.y - 1));
                let char_at_cursor = self.buf.char(pos_at_newline).prev();
                (prev_indent_len, char_at_cursor)
            };

            let desired_len = match char_at_cursor {
                Some('{') => prev_indent_len + self.config.indent_size,
                _ => prev_indent_len,
            };

            let current_indent_len = self.buf.line_indent_len(pos.y);
            let n = if pos.x < desired_len && pos.x == current_indent_len {
                desired_len - pos.x
            } else {
                let mut x = pos.x + 1;
                while x % self.config.indent_size != 0 {
                    x += 1;
                }
                x - pos.x
            };

            increase_lens.push(n);
        }

        // Insert indentations.
        let mut increase_lens_iter = increase_lens.iter();
        self.cursors.use_and_move_cursors(|c| {
            let indent_size = *increase_lens_iter.next().unwrap();
            self.buf.edit(
                c.selection(),
                &match self.config.indent_style {
                    IndentStyle::Tab => "\t".repeat(indent_size),
                    IndentStyle::Space => " ".repeat(indent_size),
                },
            )
        });
    }

    pub fn insert_str(&mut self, s: &str) {
        self.cursors
            .use_and_move_cursors(|c| self.buf.edit(c.selection(), s));
    }

    pub fn backspace(&mut self) {
        self.cursors.use_and_move_cursors(|c| {
            c.expand_left(&self.buf);
            self.buf.edit(c.selection(), "")
        });
    }

    pub fn delete(&mut self) {
        self.cursors.use_and_move_cursors(|c| {
            c.expand_right(&self.buf);
            self.buf.edit(c.selection(), "")
        });
    }
}

impl Default for Buffer {
    fn default() -> Buffer {
        Buffer::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::cursor::Cursor;

    use super::*;

    #[test]
    fn test_line_len() {
        assert_eq!(Buffer::from_text("").line_len(0), 0);
        assert_eq!(Buffer::from_text("A").line_len(0), 1);
        assert_eq!(Buffer::from_text("A\n").line_len(0), 1);
        assert_eq!(Buffer::from_text("A\nBC").line_len(1), 2);
        assert_eq!(Buffer::from_text("A\nBC\n").line_len(1), 2);
    }

    #[test]
    fn multibyte_characters() {
        let mut b = Buffer::new();
        b.insert_str("Hello 世界!");
        b.set_cursors(&[Cursor::new(0, 7)]);
        assert_eq!(b.len_chars(), 9);

        // Hello 世|界! => Hello |界!
        b.backspace();
        assert_eq!(b.text(), "Hello 界!");
        // Hello 世|界! => Hell|界!
        b.backspace();
        b.backspace();
        assert_eq!(b.text(), "Hell界!");
        // Hello 世|界! => Hell|界!
        b.insert_str("o こんにちは 世");
        assert_eq!(b.text(), "Hello こんにちは 世界!");
    }

    #[test]
    fn test_multiple_cursors1() {
        // ABC
        // おは
        // XY
        let mut b = Buffer::from_text("ABC\nおは\nXY");
        b.set_cursors(&[Cursor::new(0, 1), Cursor::new(1, 1), Cursor::new(2, 1)]);
        b.insert_str("!");
        assert_eq!(b.text(), "A!BC\nお!は\nX!Y");
        b.backspace();
        assert_eq!(b.text(), "ABC\nおは\nXY");
    }

    #[test]
    fn test_multiple_cursors2() {
        // ABC
        // おは
        // XY
        let mut b = Buffer::from_text("ABC\nおは\nXY");
        b.set_cursors(&[
            Cursor::new_selection(0, b.line_len(0), 1, 0),
            Cursor::new_selection(1, b.line_len(1), 2, 0),
        ]);
        b.insert_str("!");
        assert_eq!(b.text(), "ABC!おは!XY");
    }

    #[test]
    fn test_indentation() {
        let mut b = Buffer::from_text("");
        b.set_cursors(&[Cursor::new(0, 0)]);
        b.indent();
        assert_eq!(b.config.indent_style, IndentStyle::Space);
        assert_eq!(b.config.indent_size, 4);
        assert_eq!(b.text(), "    ");

        //     abc
        let mut b = Buffer::from_text("    abc\n");
        b.set_cursors(&[Cursor::new(1, 0)]);
        b.indent();
        assert_eq!(b.text(), "    abc\n    ");

        // __
        let mut b = Buffer::from_text("  ");
        b.set_cursors(&[Cursor::new(0, 2)]);
        b.indent();
        assert_eq!(b.text(), "    ");

        // _____
        let mut b = Buffer::from_text("     ");
        b.set_cursors(&[Cursor::new(0, 5)]);
        b.indent();
        assert_eq!(b.text(), "        ");

        // if true {
        //     while true {
        let mut b = Buffer::from_text("if true {\n    while true {\n");
        b.set_cursors(&[Cursor::new(2, 0)]);
        b.indent();
        assert_eq!(b.text(), "if true {\n    while true {\n        ");

        // if true {
        //     while true {
        // __
        let mut b = Buffer::from_text("if true {\n    while true {\n  ");
        b.set_cursors(&[Cursor::new(2, 2)]);
        b.indent();
        assert_eq!(b.text(), "if true {\n    while true {\n        ");
    }
}
