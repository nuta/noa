use crate::{
    char_iter::CharIter,
    cursor::{Cursor, Position, Range},
    find::FindIter,
    grapheme_iter::GraphemeIter,
    word_iter::{is_word_char, WordIter},
};

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

    pub fn from_reader<T: std::io::Read>(reader: T) -> std::io::Result<RawBuffer> {
        Ok(RawBuffer {
            rope: ropey::Rope::from_reader(reader)?,
        })
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
        self.char_iter(Position::new(y, 0))
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

    /// Returns a substring.
    ///
    /// # Complexity
    ///
    /// Runs in O(N) time, where N is the length of the buffer.
    pub fn substr(&self, range: Range) -> String {
        let start = self.pos_to_rope_index(range.front());
        let end = self.pos_to_rope_index(range.back());
        self.rope.slice(start..end).to_string()
    }

    /// Returns an iterator at the given position which allows traversing
    /// characters (not graphemes) in the buffer back and forth.
    pub fn char_iter(&self, pos: Position) -> CharIter<'_> {
        CharIter::new(self.rope.chars_at(self.pos_to_rope_index(pos)), self, pos)
    }

    /// Returns an iterator at the given position which allows traversing
    /// graphemes in the buffer back and forth.
    pub fn grapheme_iter(&self, pos: Position) -> GraphemeIter<'_> {
        GraphemeIter::new(self.char_iter(pos))
    }

    pub fn current_word(&self, pos: Position) -> Option<Range> {
        let mut start_iter = self.char_iter(pos);
        let mut end_iter = self.char_iter(pos);

        let mut start_pos;
        loop {
            start_pos = start_iter.last_position();
            match start_iter.prev() {
                Some(ch) if !is_word_char(ch) => break,
                Some(_) => continue,
                None => break,
            }
        }

        while let Some(ch) = end_iter.next() {
            if !is_word_char(ch) {
                break;
            }
        }

        if start_pos == end_iter.last_position() {
            return None;
        }

        Some(Range::from_positions(start_pos, end_iter.last_position()))
    }

    /// Returns an iterator at the given position which allows traversing
    /// words in the buffer back and forth.
    pub fn word_iter_from_beginning_of_word(&self, pos: Position) -> WordIter<'_> {
        WordIter::new_from_beginning_of_word(self.char_iter(pos))
    }

    /// Returns an iterator at the given position which allows traversing
    /// words in the buffer back and forth.
    pub fn word_iter_from_end_of_word(&self, pos: Position) -> WordIter<'_> {
        WordIter::new_from_end_of_word(self.char_iter(pos))
    }

    /// Returns an iterator which returns occurrences of the given string.
    pub fn find<'a, 'b>(&'a self, query: &'b str, pos: Position) -> FindIter<'a, 'b> {
        FindIter::new(self.char_iter(pos), query)
    }

    /// Replaces the text at the `range` with `new_text`.
    ///
    /// This is the only method that modifies the buffer.
    ///
    /// # Complexity
    ///
    /// According to the ropey's documentation:
    //
    /// Runs in O(M + log N) time, where N is the length of the Rope and M
    /// is the length of the range being removed/inserted.
    fn edit(&mut self, range: Range, new_text: &str) {
        let start = self.pos_to_rope_index(range.front());
        let end = self.pos_to_rope_index(range.back());

        if !(start..end).is_empty() {
            self.rope.remove(start..end);
        }

        if !new_text.is_empty() {
            self.rope.insert(start, new_text);
        }
    }

    pub fn edit_at_cursor(
        &mut self,
        current_cursor: &mut Cursor,
        past_cursors: &mut [Cursor],
        new_text: &str,
    ) {
        let range_removed = current_cursor.selection();
        let prev_back_y = current_cursor.selection().back().y;

        self.edit(range_removed, new_text);

        // Move the current cursor.
        let new_pos = Position::position_after_edit(range_removed, new_text);
        current_cursor.move_to(new_pos.y, new_pos.x);

        // Adjust past cursors.
        let y_diff = (new_pos.y as isize) - (prev_back_y as isize);
        for c in past_cursors {
            let s = c.selection_mut();

            if s.start.y == range_removed.back().y {
                s.start.x = new_pos.x + (s.start.x - range_removed.back().x);
            }
            if s.end.y == range_removed.back().y {
                s.end.x = new_pos.x + (s.end.x - range_removed.back().x);
            }

            s.start.y = ((s.start.y as isize) + y_diff) as usize;
            s.end.y = ((s.end.y as isize) + y_diff) as usize;
        }
    }

    /// Returns the character index in the rope.
    ///
    /// # Complexity
    ///
    /// Runs in O(log N) time, where N is the length of the rope.
    pub(crate) fn pos_to_rope_index(&self, pos: Position) -> usize {
        if pos.y == self.num_lines() && pos.x == 0 {
            // EOF.
            return self.rope.line_to_char(pos.y) + self.line_len(pos.y);
        }

        let column = if pos.x == std::usize::MAX {
            self.line_len(pos.y)
        } else {
            pos.x
        };

        self.rope.line_to_char(pos.y) + column
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
    fn test_substr() {
        let buffer = RawBuffer::from_text("...AB...");
        assert_eq!(buffer.substr(Range::new(0, 3, 0, 5)), "AB");

        let buffer = RawBuffer::from_text("あいうABえお");
        assert_eq!(buffer.substr(Range::new(0, 3, 0, 5)), "AB");
    }

    #[test]
    fn test_current_word() {
        let buffer = RawBuffer::from_text("");
        assert_eq!(buffer.current_word(Position::new(0, 0)), None);

        let buffer = RawBuffer::from_text("ABC ");
        assert_eq!(
            buffer.current_word(Position::new(0, 0)),
            Some(Range::new(0, 0, 0, 3))
        );
        assert_eq!(
            buffer.current_word(Position::new(0, 1)),
            Some(Range::new(0, 0, 0, 3))
        );
        assert_eq!(
            buffer.current_word(Position::new(0, 3)),
            Some(Range::new(0, 0, 0, 3))
        );
        assert_eq!(buffer.current_word(Position::new(0, 4)), None);
    }
}
