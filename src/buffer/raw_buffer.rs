use arrayvec::ArrayString;
use noa_common::int_traits::AddAndSub;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use crate::{
    cursor::{Cursor, Position, Range},
    grapheme_iter::DeprecatedGraphemeIter,
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

    /// Returns a double-ended iterator at the given position which allows
    /// traversing characters in the buffer back and forth.
    pub fn char(&self, pos: Position) -> CharIter<'_> {
        CharIter {
            iter: self.rope.chars_at(self.pos_to_rope_index(pos)),
            buf: self,
            pos,
        }
    }

    /// Returns a double-ended iterator at the given position which allows
    /// traversing words in the buffer back and forth.
    ///
    /// The iterator always returns the current word at the position first.
    pub fn word(&self, pos: Position) -> WordIter<'_> {
        WordIter {
            iter: self.char(pos),
        }
    }

    /// Returns an iterator which returns occurrences of the given string.
    pub fn find<'a, 'b>(&'a self, query: &'b str, pos: Position) -> FindIter<'a, 'b> {
        FindIter {
            chars: self.char(pos),
            query,
        }
    }

    /// Returns an iterator which returns graphemes.
    // FIXME: Stop using this.
    pub fn graphemes_in_range(&self, range: Range) -> DeprecatedGraphemeIter<'_> {
        let start = self.pos_to_rope_index(range.front());
        let end = self.pos_to_rope_index(range.back());
        DeprecatedGraphemeIter::new(&self.rope.slice(start..end))
    }

    pub fn grapheme_iter(&self, pos: Position) -> GraphemeIter<'_> {
        GraphemeIter {
            iter: self.char(pos),
        }
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

    pub fn edit_cursor(
        &mut self,
        current_cursor: &mut Cursor,
        past_cursors: &mut [Cursor],
        new_text: &str,
    ) {
        let range_removed = current_cursor.selection();

        let y_inserted = new_text.chars().filter(|c| *c == '\n').count();
        let y_deleted = range_removed.back().y - range_removed.front().y;

        // Compute the number of characters deleted/inserted in the last line.
        let string_count = new_text.chars().count();
        let x_deleted = if range_removed.front().y == range_removed.back().y {
            range_removed.back().x - range_removed.front().x
        } else {
            range_removed.end.x
        };
        let x_inserted = new_text
            .rfind('\n')
            .map(|x| string_count - x - 1)
            .unwrap_or(string_count);

        self.edit(range_removed, new_text);

        // Move the current cursor.
        let new_pos = Position::position_after_edit(range_removed, new_text);
        *current_cursor = Cursor::new(new_pos.y, new_pos.x);

        // Adjust past cursors.
        for c in past_cursors {
            let s = c.selection_mut();
            if range_removed.back().y == s.front().y {
                s.front_mut().x = s.front_mut().x.add_and_sub(x_inserted, x_deleted);
                if range_removed.back().y == s.back().y {
                    s.back_mut().x = s.back_mut().x.add_and_sub(x_inserted, x_deleted);
                }
            }

            s.start.y = s.start.y.add_and_sub(y_inserted, y_deleted);
            s.end.y = s.end.y.add_and_sub(y_inserted, y_deleted);
        }
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
        let ch = self.iter.next();
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

#[derive(Clone)]
pub struct GraphemeIter<'a> {
    iter: CharIter<'a>,
}

impl<'a> GraphemeIter<'a> {
    pub fn position(&self) -> Position {
        self.iter.pos
    }

    /// Returns the previous grapheme.
    ///
    /// # Complexity
    ///
    /// Runs in amortized O(K) time and worst-case O(log N + K) time, where K
    /// is the length in bytes of the grapheme.
    pub fn prev(&mut self) -> Option<ArrayString<16>> {
        let mut tmp = ArrayString::<4>::new();
        // Not sure if `std::usize::MAX` cause problems.
        let mut cursor = GraphemeCursor::new(std::usize::MAX, std::usize::MAX, false);
        let mut offset = 0;
        let mut char_start = self.iter.clone();
        loop {
            let (chunk, ch_len) = match self.iter.prev() {
                Some(ch) => {
                    tmp.clear();
                    tmp.push(ch);
                    (tmp.as_str(), ch.len_utf8())
                }
                None => {
                    // Reached to the EOF.
                    if offset == 0 {
                        return None;
                    } else {
                        // Return the last grapheme.

                        // Characters comes in reverse order "CBA".
                        let mut reversed = ArrayString::<16>::new();
                        while let Some(ch) = char_start.prev() {
                            reversed.push(ch);
                        }

                        //  "CBA" -> "ABC"
                        let mut grapheme = ArrayString::new();
                        for ch in reversed.chars().rev() {
                            grapheme.push(ch);
                        }

                        debug_assert!(!grapheme.is_empty());

                        self.iter = char_start;
                        return Some(grapheme);
                    }
                }
            };

            match cursor.prev_boundary(chunk, std::usize::MAX - offset - ch_len) {
                Ok(Some(n)) => {
                    // Characters comes in reverse order "CBA".
                    let mut reversed = ArrayString::<16>::new();
                    while reversed.len() < std::usize::MAX - n {
                        reversed.push(char_start.prev().unwrap());
                    }

                    //  "CBA" -> "ABC"
                    let mut grapheme = ArrayString::new();
                    for ch in reversed.chars().rev() {
                        grapheme.push(ch);
                    }

                    self.iter = char_start;
                    return Some(grapheme);
                }
                Ok(None) => {
                    // Here's unreachable since the length is set to std::usize::MAX.
                    unreachable!();
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    // Here's unreachable from `next_boundary`.
                    unreachable!();
                }
                Err(GraphemeIncomplete::InvalidOffset) => {
                    // Why?
                    panic!("GraphemeIncomplete::InvalidOffset");
                }
                Err(GraphemeIncomplete::PrevChunk) => {
                    // Continue this loop.
                }
                Err(GraphemeIncomplete::PreContext(_)) => {
                    todo!();
                }
            }

            debug_assert!(ch_len > 0);
            offset += ch_len;
        }
    }
}

impl Iterator for GraphemeIter<'_> {
    type Item = ArrayString<16>;

    /// Returns the next grapheme.
    ///
    /// # Complexity
    ///
    /// Runs in amortized O(K) time and worst-case O(log N + K) time, where K
    /// is the length in bytes of the grapheme.
    fn next(&mut self) -> Option<Self::Item> {
        let mut tmp = ArrayString::<4>::new();
        // Not sure if `std::usize::MAX` cause problems.
        let mut cursor = GraphemeCursor::new(0, std::usize::MAX, false);
        let mut offset = 0;
        let mut char_start = self.iter.clone();
        loop {
            let (chunk, ch_len) = match self.iter.next() {
                Some(ch) => {
                    tmp.clear();
                    tmp.push(ch);
                    (tmp.as_str(), ch.len_utf8())
                }
                None => {
                    // Reached to the EOF.
                    if offset == 0 {
                        return None;
                    } else {
                        // Return the last grapheme.
                        let mut grapheme = ArrayString::new();
                        for ch in char_start.by_ref() {
                            grapheme.push(ch);
                        }

                        debug_assert!(!grapheme.is_empty());

                        self.iter = char_start;
                        return Some(grapheme);
                    }
                }
            };

            match cursor.next_boundary(chunk, offset) {
                Ok(Some(n)) => {
                    let mut grapheme = ArrayString::new();
                    while grapheme.len() < n {
                        grapheme.push(char_start.next().unwrap());
                    }

                    self.iter = char_start;
                    return Some(grapheme);
                }
                Ok(None) => {
                    // Here's unreachable since the length is set to std::usize::MAX.
                    unreachable!();
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    // Continue this loop.
                }
                Err(GraphemeIncomplete::InvalidOffset) => {
                    // Why?
                    panic!("GraphemeIncomplete::InvalidOffset");
                }
                Err(GraphemeIncomplete::PrevChunk) => {
                    // Here's unreachable from `next_boundary`.
                    unreachable!();
                }
                Err(GraphemeIncomplete::PreContext(_)) => {
                    todo!();
                }
            }

            debug_assert!(ch_len > 0);
            offset += ch_len;
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Word<'a> {
    buf: &'a RawBuffer,
    range: Range,
}

impl<'a> Word<'a> {
    pub fn range(&self) -> Range {
        self.range
    }

    pub fn text(&self) -> String {
        self.buf.substr(self.range)
    }
}

#[derive(Clone)]
pub struct WordIter<'a> {
    iter: CharIter<'a>,
}

impl<'a> Iterator for WordIter<'a> {
    type Item = Word<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // Handle the EOF case.
        self.iter.clone().next()?;

        // If the iterator points to the end of a word, move the iterator back
        // so that it returns the word.
        let mut end = self.iter.clone();
        match end.prev() {
            Some(prev_ch) if !prev_ch.is_ascii_whitespace() => {}
            _ => {
                end = self.iter.clone();
            }
        }

        // Skip whitespaces.
        let first_ch = loop {
            match end.next() {
                Some(ch) if !ch.is_ascii_whitespace() => {
                    break ch;
                }
                Some(_) => {
                    // Whitespaces and tabs. Keep skipping.
                    continue;
                }
                None => {
                    return None;
                }
            }
        };

        // Determine the character category.
        let is_same_category = match first_ch {
            ch if ch.is_ascii_alphanumeric() => |c: char| c.is_ascii_alphanumeric(),
            ch if ch.is_ascii_punctuation() => |c: char| c.is_ascii_punctuation(),
            _ => |c: char| !c.is_ascii_whitespace(),
        };

        // Find the beginning of the word.
        let mut start = end.clone();
        while let Some(ch) = start.prev() {
            if !is_same_category(ch) {
                start.next();
                break;
            }
        }

        // Find the end of the word.
        while let Some(ch) = end.next() {
            if !is_same_category(ch) {
                end.prev();
                break;
            }
        }

        self.iter = end.clone();
        // Move the iterator to the next to next whitespace (not at the end of
        // the word).
        self.iter.next();

        Some(Word {
            buf: self.iter.buf,
            range: Range::from_positions(start.position(), end.position()),
        })
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
    fn test_substr() {
        let buffer = RawBuffer::from_text("...AB...");
        assert_eq!(buffer.substr(Range::new(0, 3, 0, 5)), "AB");

        let buffer = RawBuffer::from_text("あいうABえお");
        assert_eq!(buffer.substr(Range::new(0, 3, 0, 5)), "AB");
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

        let buffer = RawBuffer::from_text("XYZ");
        let mut iter = buffer.char(Position::new(0, 1));
        assert_eq!(iter.prev(), Some('X'));
    }

    #[test]
    fn test_grapheme_iter_next() {
        let buffer = RawBuffer::from_text("ABC");
        let mut iter = buffer.grapheme_iter(Position::new(0, 0));
        assert_eq!(iter.next().map(|s| s.to_string()), Some("A".to_string()));
        assert_eq!(iter.next().map(|s| s.to_string()), Some("B".to_string()));
        assert_eq!(iter.next().map(|s| s.to_string()), Some("C".to_string()));
        assert_eq!(iter.next().map(|s| s.to_string()), None);

        let buffer = RawBuffer::from_text("あaい");
        let mut iter = buffer.grapheme_iter(Position::new(0, 0));
        assert_eq!(iter.next().map(|s| s.to_string()), Some("あ".to_string()));
        assert_eq!(iter.next().map(|s| s.to_string()), Some("a".to_string()));
        assert_eq!(iter.next().map(|s| s.to_string()), Some("い".to_string()));
        assert_eq!(iter.next().map(|s| s.to_string()), None);

        // A grapheme ("ka" in Katakana with Dakuten), consists of U+304B U+3099.
        let buffer = RawBuffer::from_text("\u{304b}\u{3099}");
        let mut iter = buffer.grapheme_iter(Position::new(0, 0));
        assert_eq!(
            iter.next().map(|s| s.to_string()),
            Some("\u{304b}\u{3099}".to_string())
        );
        assert_eq!(iter.next().map(|s| s.to_string()), None);

        // A grapheme ("u" with some marks), consists of U+0075 U+0308 U+0304.
        let buffer = RawBuffer::from_text("\u{0075}\u{0308}\u{0304}BC");
        let mut iter = buffer.grapheme_iter(Position::new(0, 0));
        assert_eq!(
            iter.next().map(|s| s.to_string()),
            Some("\u{0075}\u{0308}\u{0304}".to_string())
        );
        assert_eq!(iter.next().map(|s| s.to_string()), Some("B".to_string()));
        assert_eq!(iter.next().map(|s| s.to_string()), Some("C".to_string()));
        assert_eq!(iter.next().map(|s| s.to_string()), None);
    }

    #[test]
    fn test_grapheme_iter_prev() {
        let buffer = RawBuffer::from_text("ABC");
        let mut iter = buffer.grapheme_iter(Position::new(0, 3));
        assert_eq!(iter.prev().map(|s| s.to_string()), Some("C".to_string()));
        assert_eq!(iter.prev().map(|s| s.to_string()), Some("B".to_string()));
        assert_eq!(iter.prev().map(|s| s.to_string()), Some("A".to_string()));
        assert_eq!(iter.prev().map(|s| s.to_string()), None);

        let buffer = RawBuffer::from_text("あaい");
        let mut iter = buffer.grapheme_iter(Position::new(0, 3));
        assert_eq!(iter.prev().map(|s| s.to_string()), Some("い".to_string()));
        assert_eq!(iter.prev().map(|s| s.to_string()), Some("a".to_string()));
        assert_eq!(iter.prev().map(|s| s.to_string()), Some("あ".to_string()));
        assert_eq!(iter.prev().map(|s| s.to_string()), None);

        // A grapheme ("ka" in Katakana with Dakuten), consists of U+304B U+3099.
        let buffer = RawBuffer::from_text("\u{304b}\u{3099}");
        let mut iter = buffer.grapheme_iter(Position::new(0, 2));
        assert_eq!(
            iter.prev().map(|s| s.to_string()),
            Some("\u{304b}\u{3099}".to_string())
        );
        assert_eq!(iter.prev().map(|s| s.to_string()), None);

        // A grapheme ("u" with some marks), consists of U+0075 U+0308 U+0304.
        let buffer = RawBuffer::from_text("\u{0075}\u{0308}\u{0304}BC");
        let mut iter = buffer.grapheme_iter(Position::new(0, 5));
        assert_eq!(iter.prev().map(|s| s.to_string()), Some("C".to_string()));
        assert_eq!(iter.prev().map(|s| s.to_string()), Some("B".to_string()));
        assert_eq!(
            iter.prev().map(|s| s.to_string()),
            Some("\u{0075}\u{0308}\u{0304}".to_string())
        );
        assert_eq!(iter.prev().map(|s| s.to_string()), None);
    }

    #[test]
    fn test_word() {
        let buffer = RawBuffer::from_text("");
        let mut iter = buffer.word(Position::new(0, 0));
        assert_eq!(iter.next().map(|w| w.range()), None);

        let buffer = RawBuffer::from_text("A");
        let mut iter = buffer.word(Position::new(0, 0));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 0, 0, 1)));
        assert_eq!(iter.next().map(|w| w.range()), None);

        let buffer = RawBuffer::from_text("ABC DEF");
        let mut iter = buffer.word(Position::new(0, 3));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 0, 0, 3)));

        let buffer = RawBuffer::from_text("abc WXYZ   12");
        let mut iter = buffer.word(Position::new(0, 0));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 0, 0, 3)));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 4, 0, 8)));
        assert_eq!(
            iter.next().map(|w| w.range()),
            Some(Range::new(0, 11, 0, 13))
        );
        assert_eq!(iter.next().map(|w| w.range()), None);

        let mut iter = buffer.word(Position::new(0, 5));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 4, 0, 8)));

        let mut iter = buffer.word(Position::new(0, 8));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 4, 0, 8)));
    }
}
