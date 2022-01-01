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

    /// Returns the number of lines in the buffer.
    fn num_lines(&self) -> usize {
        self.cached_num_lines
    }

    /// Turns the whole buffer into a string.
    fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Returns a double-ended iterator at the given position which allows
    /// traversing characters in the buffer back and forth.
    pub fn char(&mut self, pos: Position) -> CharIter<'_> {
        CharIter {
            iter: self.rope.chars_at(self.index_in_rope(pos)),
        }
    }

    /// Replaces the text at the `range` with `new_text`.
    pub fn edit(&mut self, range: Range, new_text: &str) {
        let start = self.index_in_rope(range.front());
        let end = self.index_in_rope(range.back());
        self.rope.remove(start..end);
        self.rope.insert(start, new_text);
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

impl Iterator for CharIter<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.iter.next()
    }
}

impl DoubleEndedIterator for CharIter<'_> {
    fn next_back(&mut self) -> Option<char> {
        self.iter.prev()
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
}
