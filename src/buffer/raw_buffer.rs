use crate::cursor::{Position, Range};

/// An internal buffer implementation supporting primitive operations required
/// by the editor.
///
/// This object offers a cheap `clone`-ing thanks to the underlying data sturcture
/// called *rope*. It makes significantly easy to implement undo/redo operations.
#[derive(Clone)]
pub struct RawBuffer {
    /// The inner buffer data structure.
    rope: ropey::Rope,
}

impl RawBuffer {
    pub fn new() -> RawBuffer {
        RawBuffer {
            rope: ropey::Rope::new(),
        }
    }

    pub fn from_text(text: &str) -> RawBuffer {
        RawBuffer {
            rope: ropey::Rope::from_str(text),
        }
    }

    pub fn rope(&self) -> &ropey::Rope {
        &self.rope
    }

    pub fn write_to(&self, writer: impl std::io::Write) -> std::io::Result<()> {
        self.rope.write_to(writer)
    }

    /// Returns the number of lines in the buffer.
    ///
    /// # Complexity
    ///
    /// Runs in O(1) time.
    pub fn num_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Returns the number of characters in the buffer.
    ///
    /// # Complexity
    ///
    /// Runs in O(1) time.
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Returns the number of characters in a line except new line characters.
    ///
    /// # Complexity
    ///
    /// Runs in O(log N) time, where N is the length of the buffer.
    pub fn line_len(&self, y: usize) -> usize {
        if y == self.num_lines() {
            0
        } else {
            let line = self.rope.line(y);

            // The `line` contains newline characters so we need to subtract them.
            let num_newline_chars = line
                .chunks()
                .last()
                .map(|chunk| chunk.matches(|c| c == '\n' || c == '\r').count())
                .unwrap_or(0);

            line.len_chars() - num_newline_chars
        }
    }

    /// Returns the number of indentation characters in a line.
    ///
    /// # Complexity
    ///
    /// Runs in O(M + log N) time, where N is the length of the rope and M is
    /// the length of the line.
    pub fn line_indent_len(&self, y: usize) -> usize {
        self.char(Position::new(y, 0))
            .take_while(|c| *c == ' ' || *c == '\t')
            .count()
    }

    /// Turns the whole buffer into a string.
    ///
    /// # Complexity
    ///
    /// Runs in O(N) time, where N is the length of the buffer.
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Returns a double-ended iterator at the given position which allows
    /// traversing characters in the buffer back and forth.
    pub fn char(&self, pos: Position) -> CharIter<'_> {
        CharIter {
            iter: self.rope.chars_at(self.pos_to_rope_index(pos)),
            buf: self,
            pos,
        }
    }

    /// Returns an iterator which returns occurrences of the given string.
    pub fn find<'a, 'b>(&'a self, query: &'b str, pos: Position) -> FindIter<'a, 'b> {
        FindIter {
            chars: self.char(pos),
            query,
        }
    }

    /// Replaces the text at the `range` with `new_text`. Returns the cursor
    /// position after the replacement.
    ///
    /// This is the only method that modifies the buffer.
    ///
    /// # Complexity
    ///
    /// According to the ropey's documentation:
    //
    /// Runs in O(M + log N) time, where N is the length of the Rope and M
    /// is the length of the range being removed/inserted.
    pub fn edit(&mut self, range: Range, new_text: &str) -> Position {
        let start = self.pos_to_rope_index(range.front());
        let end = self.pos_to_rope_index(range.back());

        if !(start..end).is_empty() {
            self.rope.remove(start..end);
        }

        if !new_text.is_empty() {
            self.rope.insert(start, new_text);
        }

        Position::position_after_edit(range, new_text)
    }

    /// Returns the character index in the rope.
    ///
    /// # Complexity
    ///
    /// Runs in O(log N) time, where N is the length of the rope.
    fn pos_to_rope_index(&self, pos: Position) -> usize {
        let column = if pos.x == std::usize::MAX {
            self.line_len(pos.y)
        } else {
            pos.x
        };

        self.rope.line_to_char(pos.y) + column
    }

    /// Returns the position of the given rope character index.
    ///
    /// # Complexity
    ///
    /// Runs in O(log N) time, where N is the length of the rope.
    fn _rope_index_to_pos(&self, char_index: usize) -> Position {
        let y = self.rope.char_to_line(char_index);
        let x = char_index - self.rope.line_to_char(y);

        Position::new(y, x)
    }
}

impl Default for RawBuffer {
    fn default() -> RawBuffer {
        RawBuffer::new()
    }
}

impl PartialEq for RawBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.rope == other.rope
    }
}

#[derive(Clone)]
pub struct CharIter<'a> {
    iter: ropey::iter::Chars<'a>,
    buf: &'a RawBuffer,
    pos: Position,
}

impl<'a> CharIter<'a> {
    pub fn position(&self) -> Position {
        self.pos
    }
    /// Returns the previous character.
    ///
    /// # Complexity
    ///
    /// From ropey's documentation:
    ///
    /// > Runs in amortized O(1) time and worst-case O(log N) time.
    pub fn prev(&mut self) -> Option<char> {
        let ch = self.iter.prev();
        match ch {
            Some('\n') => {
                self.pos.y -= 1;
                self.pos.x = self.buf.line_len(self.pos.y);
            }
            Some('\r') => {
                // Do nothing.
            }
            Some(_) => {
                self.pos.x -= 1;
            }
            None => {
                // Do nothing.
            }
        }
        ch
    }
}

impl Iterator for CharIter<'_> {
    type Item = char;

    /// Returns the next character.
    ///
    /// # Complexity
    ///
    /// From ropey's documentation:
    ///
    /// > Runs in amortized O(1) time and worst-case O(log N) time.
    fn next(&mut self) -> Option<char> {
        let ch = self.iter.prev();
        match ch {
            Some('\n') => {
                self.pos.y += 1;
                self.pos.x = 0;
            }
            Some('\r') => {
                // Do nothing.
            }
            Some(_) => {
                self.pos.x += 1;
            }
            None => {
                // Do nothing.
            }
        }
        ch
    }
}

pub struct FindIter<'a, 'b> {
    chars: CharIter<'a>,
    query: &'b str,
}

impl<'a, 'b> Iterator for FindIter<'a, 'b> {
    type Item = Position;

    fn next(&mut self) -> Option<Position> {
        let mut query_iter = self.query.chars();
        let mut buf_iter = self.chars.clone();
        let pos = buf_iter.position();

        self.chars.next();

        loop {
            match (buf_iter.next(), query_iter.next()) {
                (Some(a), Some(b)) if a != b => {
                    return None;
                }
                (None, Some(_)) => {
                    // Reached to EOF.
                    return None;
                }
                (_, None) => {
                    return Some(pos);
                }
                (Some(_), Some(_)) => {
                    // Continue comparing the next characters...
                }
            }
        }
    }
}

impl<'a, 'b> DoubleEndedIterator for FindIter<'a, 'b> {
    fn next_back(&mut self) -> Option<Position> {
        let mut query_iter = self.query.chars();
        let mut buf_iter = self.chars.clone();
        let pos = buf_iter.position();

        self.chars.prev();

        loop {
            match (buf_iter.next(), query_iter.next()) {
                (Some(a), Some(b)) if a != b => {
                    return None;
                }
                (None, Some(_)) => {
                    // Reached to EOF.
                    return None;
                }
                (_, None) => {
                    return Some(pos);
                }
                (Some(_), Some(_)) => {
                    // Continue comparing the next characters...
                }
            }
        }
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
        let mut buffer = RawBuffer::from_text("ABCDEFG");
        buffer.edit(Range::new(0, 1, 0, 1), "");
        assert_eq!(buffer.text(), "ABCDEFG");

        buffer.edit(Range::new(0, 1, 0, 3), "");
        assert_eq!(buffer.text(), "ADEFG");
    }

    #[test]
    fn test_char() {
        let buffer = RawBuffer::from_text("XY\n123");
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
