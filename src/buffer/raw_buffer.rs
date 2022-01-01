use crate::cursor::{Position, Range};

/// An internal buffer implementation supporting primitive operations required
/// by the editor.
///
/// This object offers a cheap `clone`-ing thanks to the underlying data sturcture
/// called *rope*. It makes significantly easy to implement undo/redo operations.
#[derive(Clone)]
pub struct RawBuffer {
    /// The inner buffer data structure.
    pub rope: ropey::Rope,
    /// The `len_lines()` is expensive, so we cache it.
    cached_num_lines: usize,
}

impl RawBuffer {
    pub fn new() -> RawBuffer {
        RawBuffer {
            rope: ropey::Rope::new(),
            cached_num_lines: 0,
        }
    }

    #[cfg(test)]
    pub fn from_str(text: &str) -> RawBuffer {
        let mut buffer = RawBuffer {
            rope: ropey::Rope::from_str(text),
            cached_num_lines: 0,
        };

        buffer.after_update();
        buffer
    }

    /// Returns the number of lines in the buffer.
    pub fn num_lines(&self) -> usize {
        self.cached_num_lines
    }

    /// Turns the whole buffer into a string.
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Returns a double-ended iterator at the given position which allows
    /// traversing characters in the buffer back and forth.
    /// # Complexity
    ///
    /// From ropey's documentation:
    ///
    /// > Runs in amortized O(1) time and worst-case O(log N) time.
    pub fn char(&self, pos: Position) -> CharIter<'_> {
        CharIter {
            iter: self.rope.chars_at(self.index_in_rope(pos)),
        }
    }

    /// Replaces the text at the `range` with `new_text`.
    ///
    /// # Complexity
    ///
    /// According to the ropey's documentation:
    //
    /// Runs in O(M + log N) time, where N is the length of the Rope and M
    /// is the length of the range being removed/inserted.
    pub fn edit(&mut self, range: Range, new_text: &str) {
        let start = self.index_in_rope(range.front());
        let end = self.index_in_rope(range.back());

        self.rope.remove(start..end);
        self.rope.insert(start, new_text);

        self.after_update();
    }

    fn after_update(&mut self) {
        self.cached_num_lines = self.rope.len_lines();
    }

    /// Returns the number of characters in a line except new line characters.
    fn line_len(&self, line: usize) -> usize {
        if line == self.num_lines() {
            0
        } else {
            self.rope.line(line).len_chars()
        }
    }

    /// Returns the character index in the rope.
    fn index_in_rope(&self, pos: Position) -> usize {
        let column = if pos.x == std::usize::MAX {
            self.line_len(pos.y)
        } else {
            pos.x
        };

        self.rope.line_to_char(pos.y) + column
    }
}

pub struct CharIter<'a> {
    iter: ropey::iter::Chars<'a>,
}

impl<'a> CharIter<'a> {
    pub fn prev(&mut self) -> Option<char> {
        self.iter.prev()
    }
}

impl Iterator for CharIter<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.iter.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insertion() {
        let mut buffer = RawBuffer::new();
        buffer.edit(Range::new(0, 0, 0, 0), "ABG");
        assert_eq!(buffer.text(), "ABG");

        buffer.edit(Range::new(0, 2, 0, 2), "CDEF");
        assert_eq!(buffer.text(), "ABCDEFG");
    }

    #[test]
    fn test_deletion() {
        let mut buffer = RawBuffer::from_str("ABCDEFG");
        buffer.edit(Range::new(0, 1, 0, 1), "");
        assert_eq!(buffer.text(), "ABCDEFG");

        buffer.edit(Range::new(0, 1, 0, 3), "");
        assert_eq!(buffer.text(), "ADEFG");
    }

    #[test]
    fn test_char() {
        let buffer = RawBuffer::from_str("XY\n123");
        let mut iter = buffer.char(Position::new(1, 1));
        assert_eq!(iter.next(), Some('2'));
        assert_eq!(iter.prev(), Some('2'));
        assert_eq!(iter.prev(), Some('1'));
        assert_eq!(iter.prev(), Some('\n'));
        assert_eq!(iter.prev(), Some('Y'));
        assert_eq!(iter.prev(), Some('X'));
        assert_eq!(iter.prev(), None);
        assert_eq!(iter.next(), Some('X'));
        assert_eq!(iter.next(), Some('Y'));
        assert_eq!(iter.next(), Some('\n'));
        assert_eq!(iter.next(), Some('1'));
    }
}
