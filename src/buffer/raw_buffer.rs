use std::{cmp::min, fmt};

use crate::{
    char_iter::CharIter,
    cursor::{Position, Range},
    find::FindIter,
    grapheme_iter::{BidirectionalGraphemeIter, GraphemeIter},
    paragraph_iter::ParagraphIter,
    reflow_iter::ReflowIter,
    word_iter::{is_word_char, WordIter},
};

/// An internal immutable buffer, being used as a snapshot.
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

    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
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

    pub fn clamp_position(&self, pos: Position) -> Position {
        Position::new(
            min(pos.y, self.num_lines().saturating_sub(1)),
            min(pos.x, self.line_len(pos.y)),
        )
    }

    pub fn clamp_range(&self, range: Range) -> Range {
        let mut r = range;
        r.start.y = min(r.start.y, self.num_lines().saturating_sub(1));
        r.end.y = min(r.end.y, self.num_lines().saturating_sub(1));
        r.start.x = min(r.start.x, self.line_len(r.start.y));
        r.end.x = min(r.end.x, self.line_len(r.end.y));
        r
    }

    pub fn is_valid_position(&self, pos: Position) -> bool {
        self.clamp_position(pos) == pos
    }

    pub fn is_valid_range(&self, range: Range) -> bool {
        self.is_valid_position(range.start) && self.is_valid_position(range.end)
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
        let start = self.pos_to_char_index(range.front());
        let end = self.pos_to_char_index(range.back());
        self.rope.slice(start..end).to_string()
    }

    /// Returns the text in a line excluding newline character(s).
    ///
    /// # Complexity
    ///
    /// Runs in O(N) time, where N is the length of the line.
    pub fn line_text(&self, y: usize) -> String {
        self.substr(Range::new(y, 0, y, self.line_len(y)))
    }

    /// Returns an iterator at the given position which allows traversing
    /// characters (not graphemes) in the buffer back and forth.
    pub fn char_iter(&self, pos: Position) -> CharIter<'_> {
        CharIter::new(self.rope.chars_at(self.pos_to_char_index(pos)), self, pos)
    }

    /// Returns an iterator at the given position which allows traversing
    /// graphemes in the buffer.
    pub fn grapheme_iter(&self, pos: Position) -> GraphemeIter<'_> {
        GraphemeIter::new(self, pos)
    }

    /// Returns an iterator at the given position which allows traversing
    /// graphemes in the buffer back and forth.
    ///
    /// Prefer using this method over `grapheme_iter` if you don't
    /// need to move a iterator backwards.
    pub fn bidirectional_grapheme_iter(&self, pos: Position) -> BidirectionalGraphemeIter<'_> {
        BidirectionalGraphemeIter::new(self, pos)
    }

    /// Returns an iterator at the given position which returns graphemes in
    /// the screen.
    pub fn reflow_iter(
        &self,
        pos: Position,
        screen_width: usize,
        tab_width: usize,
    ) -> ReflowIter<'_> {
        ReflowIter::new(self, pos, None, screen_width, tab_width)
    }

    pub fn paragraph_iter(
        &self,
        pos: Position,
        screen_width: usize,
        tab_width: usize,
    ) -> ParagraphIter<'_> {
        ParagraphIter::new(self, pos, screen_width, tab_width)
    }

    /// Returns the current word range.
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

        for ch in end_iter.by_ref() {
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
    pub fn find_iter<'a, 'b>(&'a self, query: &'b str, pos: Position) -> FindIter<'a, 'b> {
        FindIter::new(self.char_iter(pos), query)
    }

    pub(crate) fn rope_slice(&self, range: Range) -> ropey::RopeSlice<'_> {
        let start = self.pos_to_char_index(range.front());
        let end = self.pos_to_char_index(range.back());
        self.rope.slice(start..end)
    }

    /// Returns the character index in the rope.
    ///
    /// # Complexity
    ///
    /// Runs in O(log N) time, where N is the length of the rope.
    pub fn pos_to_char_index(&self, pos: Position) -> usize {
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

    pub fn pos_to_byte_index(&self, pos: Position) -> usize {
        self.rope.char_to_byte(self.pos_to_char_index(pos))
    }

    pub fn char_index_to_pos(&self, char_index: usize) -> Position {
        let y = self.rope.char_to_line(char_index);
        let x = char_index - self.rope.line_to_char(y);
        Position { y, x }
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

impl fmt::Debug for RawBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RawBuffer {{ num_lines: {} }}", self.num_lines())
    }
}

impl From<ropey::Rope> for RawBuffer {
    fn from(rope: ropey::Rope) -> RawBuffer {
        RawBuffer { rope }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
