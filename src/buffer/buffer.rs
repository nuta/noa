use crate::{
    cursor::{Cursor, CursorSet, Position},
    raw_buffer::RawBuffer,
};

pub struct Buffer {
    buf: RawBuffer,
    cursors: CursorSet,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            buf: RawBuffer::new(),
            cursors: CursorSet::new(),
        }
    }

    pub fn from_str(text: &str) -> Buffer {
        Buffer {
            buf: RawBuffer::from_str(text),
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
        assert_eq!(Buffer::from_str("").line_len(0), 0);
        assert_eq!(Buffer::from_str("A").line_len(0), 1);
        assert_eq!(Buffer::from_str("A\n").line_len(0), 1);
        assert_eq!(Buffer::from_str("A\nBC").line_len(1), 2);
        assert_eq!(Buffer::from_str("A\nBC\n").line_len(1), 2);
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
        let mut b = Buffer::from_str("ABC\nおは\nXY");
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
        let mut b = Buffer::from_str("ABC\nおは\nXY");
        b.set_cursors(&[
            Cursor::new_selection(0, b.line_len(0), 1, 0),
            Cursor::new_selection(1, b.line_len(1), 2, 0),
        ]);
        b.insert_str("!");
        assert_eq!(b.text(), "ABC!おは!XY");
    }
}
