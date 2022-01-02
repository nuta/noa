use crate::{
    cursor::{CursorSet, Position},
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

    #[cfg(test)]
    pub fn from_str(text: &str) -> Buffer {
        Buffer {
            buf: RawBuffer::from_str(text),
            ..Default::default()
        }
    }

    pub fn len_chars(&self) -> usize {
        self.buf.len_chars()
    }

    pub fn text(&self) -> String {
        self.buf.text()
    }

    pub fn cursors_mut(&mut self) -> &mut CursorSet {
        &mut self.cursors
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
    fn multibyte_characters() {
        let mut b = Buffer::new();
        b.insert_str("Hello 世界!");
        b.cursors_mut().set_cursors(&[Cursor::new(0, 7)]);
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
        let mut b = Buffer::from_str("BC");
        b.insert_str("A");
        assert_eq!(b.text(), "ABC");
    }
}
