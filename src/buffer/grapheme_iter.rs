use arrayvec::ArrayString;
use ropey::str_utils::byte_to_char_idx;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use crate::{
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
};

/// Finds the next grapheme boundary after the given char position.
///
/// Based on <https://github.com/cessen/led/blob/8a9388e8166e3e076f8bc8e256327bee9cd177b7/src/graphemes.rs>.
/// Apache 2.0 or MIT licensed.
pub fn next_grapheme_boundary(slice: &ropey::RopeSlice, char_idx: usize) -> Option<usize> {
    // Bounds check
    debug_assert!(char_idx <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

    // Find the next grapheme cluster boundary.
    loop {
        match gc.next_boundary(chunk, chunk_byte_idx) {
            Ok(None) => return None,
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                return Some(chunk_char_idx + tmp);
            }
            Err(GraphemeIncomplete::NextChunk) => {
                chunk_byte_idx += chunk.len();
                let (a, _, c, _) = slice.chunk_at_byte(chunk_byte_idx);
                chunk = a;
                chunk_char_idx = c;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                gc.provide_context(ctx_chunk, n - ctx_chunk.len());
            }
            _ => unreachable!(),
        }
    }
}

/// Finds the previous grapheme boundary before the given char position.
///
/// Based on <https://github.com/cessen/led/blob/8a9388e8166e3e076f8bc8e256327bee9cd177b7/src/graphemes.rs>.
/// Apache 2.0 or MIT licensed.
pub fn prev_grapheme_boundary(slice: &ropey::RopeSlice, char_idx: usize) -> Option<usize> {
    // Bounds check
    debug_assert!(char_idx <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

    // Find the previous grapheme cluster boundary.
    loop {
        match gc.prev_boundary(chunk, chunk_byte_idx) {
            Ok(None) => return None,
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                return Some(chunk_char_idx + tmp);
            }
            Err(GraphemeIncomplete::PrevChunk) => {
                let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_idx - 1);
                chunk = a;
                chunk_byte_idx = b;
                chunk_char_idx = c;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                gc.provide_context(ctx_chunk, n - ctx_chunk.len());
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Clone)]
pub struct GraphemeIter<'a> {
    buf: &'a RawBuffer,
    next_pos: Position,
    last_pos: Position,
}

impl<'a> GraphemeIter<'a> {
    pub fn new(buf: &'a RawBuffer, pos: Position) -> GraphemeIter<'a> {
        GraphemeIter {
            buf,
            next_pos: pos,
            last_pos: pos,
        }
    }

    pub fn next_position(&self) -> Position {
        self.next_pos
    }

    pub fn last_position(&self) -> Position {
        self.last_pos
    }

    /// Returns the previous grapheme.
    ///
    /// # Complexity
    ///
    /// Runs in amortized O(K) time and worst-case O(log N + K) time, where K
    /// is the length in bytes of the grapheme.
    pub fn prev(&mut self) -> Option<ArrayString<16>> {
        let slice = self.buf.rope().slice(..);
        let char_index = prev_grapheme_boundary(&slice, self.buf.pos_to_char_index(self.next_pos))?;
        let pos = self.buf.char_index_to_pos(char_index);
        let grapheme = self.buf.substr(Range::from_positions(self.next_pos, pos));
        self.last_pos = self.next_pos;
        self.next_pos = pos;
        Some(ArrayString::from(&grapheme).unwrap())
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
        let slice = self.buf.rope().slice(..);
        let char_index = next_grapheme_boundary(&slice, self.buf.pos_to_char_index(self.next_pos))?;
        let pos = self.buf.char_index_to_pos(char_index);
        let grapheme = self.buf.substr(Range::from_positions(self.next_pos, pos));
        self.last_pos = self.next_pos;
        self.next_pos = pos;
        Some(ArrayString::from(&grapheme).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::raw_buffer::RawBuffer;

    use super::*;

    #[test]
    fn test_grapheme_iter_next() {
        // ABC
        // XY
        let buffer = RawBuffer::from_text("ABC\nXY");
        let mut iter = buffer.grapheme_iter(Position::new(0, 0));

        assert_eq!(iter.next_position(), Position::new(0, 0));
        assert_eq!(iter.next(), Some(ArrayString::from_str("A").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 1));
        assert_eq!(iter.next(), Some(ArrayString::from_str("B").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 2));
        assert_eq!(iter.next(), Some(ArrayString::from_str("C").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 3));
        assert_eq!(iter.next(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.next_position(), Position::new(1, 0));
        assert_eq!(iter.next(), Some(ArrayString::from_str("X").unwrap()));
        assert_eq!(iter.next_position(), Position::new(1, 1));
    }

    #[test]
    fn test_grapheme_iter_prev() {
        // ABC
        // XY
        let buffer = RawBuffer::from_text("ABC\nXY");
        let mut iter = buffer.grapheme_iter(Position::new(1, 0));

        assert_eq!(iter.next_position(), Position::new(1, 0));
        assert_eq!(iter.prev(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 3));
        assert_eq!(iter.prev(), Some(ArrayString::from_str("C").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 2));
    }

    #[test]
    fn test_grapheme_iter_next_with_complicated_emojis() {
        // Note: the emoji is 3-characters wide: U+1F469 U+200D U+1F52C.
        let buffer = RawBuffer::from_text("a👩‍🔬");
        let mut iter = buffer.grapheme_iter(Position::new(0, 0));

        assert_eq!(iter.next_position(), Position::new(0, 0));
        assert_eq!(iter.next(), Some(ArrayString::from_str("a").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 1));
        assert_eq!(iter.next(), Some(ArrayString::from_str("👩‍🔬").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 4));
    }

    #[test]
    fn test_grapheme_iter_prev_with_complicated_emojis() {
        // Note: the emoji is 3-characters wide: U+1F469 U+200D U+1F52C.
        let buffer = RawBuffer::from_text("a👩‍🔬");
        let mut iter = buffer.grapheme_iter(Position::new(0, 4));

        assert_eq!(iter.next_position(), Position::new(0, 4));
        assert_eq!(iter.prev(), Some(ArrayString::from_str("👩‍🔬").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 1));
        assert_eq!(iter.prev(), Some(ArrayString::from_str("a").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 0));
    }

    #[test]
    fn test_grapheme_iter_newline() {
        // ABC
        // XY
        let buffer = RawBuffer::from_text("ABC\nXY");
        let mut iter = buffer.grapheme_iter(Position::new(0, 3));

        assert_eq!(iter.next_position(), Position::new(0, 3));
        assert_eq!(iter.next(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.next_position(), Position::new(1, 0));
        assert_eq!(iter.prev(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 3));
        assert_eq!(iter.next(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.next_position(), Position::new(1, 0));
    }

    #[test]
    fn test_grapheme_iter_next2() {
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
    fn test_grapheme_iter_prev2() {
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

        // A grapheme ("か" with dakuten), consists of U+304B U+3099.
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
}
