use ropey::{str_utils::byte_to_char_idx, RopeSlice};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use crate::{cursor::Position, raw_buffer::RawBuffer};

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
pub struct BidirectionalGraphemeIter<'a> {
    buf: &'a RawBuffer,
    slice: RopeSlice<'a>,
    next_char_index: usize,
    next_char_pos: Position,
}

impl<'a> BidirectionalGraphemeIter<'a> {
    pub fn new(buf: &'a RawBuffer, pos: Position) -> BidirectionalGraphemeIter<'a> {
        let char_index = buf.pos_to_char_index(pos);
        BidirectionalGraphemeIter {
            buf,
            slice: buf.rope().slice(..),
            next_char_index: char_index,
            next_char_pos: pos,
        }
    }

    pub fn next_position(&self) -> Position {
        self.next_char_pos
    }

    /// Returns the previous grapheme.
    ///
    /// # Complexity
    ///
    /// Runs in amortized O(K) time and worst-case O(log N + K) time, where K
    /// is the length in bytes of the grapheme.
    pub fn prev(&mut self) -> Option<(Position, String)> {
        let char_index = prev_grapheme_boundary(&self.slice, self.next_char_index)?;
        let grapheme = self
            .slice
            .slice(char_index..self.next_char_index)
            .to_string();

        for ch in grapheme.chars().rev() {
            match ch {
                '\n' => {
                    self.next_char_pos.y -= 1;
                    self.next_char_pos.x = self.buf.line_len(self.next_char_pos.y);
                }
                '\r' => {
                    // Do nothing.
                }
                _ => {
                    self.next_char_pos.x = self.next_char_pos.x.saturating_sub(1);
                }
            }
        }

        self.next_char_index = char_index;
        Some((self.next_char_pos, grapheme))
    }
}

impl Iterator for BidirectionalGraphemeIter<'_> {
    type Item = (Position, String);

    /// Returns the next grapheme.
    ///
    /// # Complexity
    ///
    /// Runs in amortized O(K) time and worst-case O(log N + K) time, where K
    /// is the length in bytes of the grapheme.
    fn next(&mut self) -> Option<Self::Item> {
        let char_index = next_grapheme_boundary(&self.slice, self.next_char_index)?;
        let grapheme = self
            .slice
            .slice(self.next_char_index..char_index)
            .to_string();

        let pos = self.next_char_pos;
        for ch in grapheme.chars() {
            match ch {
                '\n' => {
                    self.next_char_pos.y += 1;
                    self.next_char_pos.x = 0;
                }
                '\r' => {
                    // Do nothing.
                }
                _ => {
                    self.next_char_pos.x += 1;
                }
            }
        }

        self.next_char_index = char_index;
        Some((pos, grapheme))
    }
}

/// Another grapheme iterator implementation faster than `BidirectionalGraphemeIter`.
#[derive(Clone)]
pub struct GraphemeIter<'a> {
    rope: ropey::RopeSlice<'a>,
    chunk: &'a str,
    chunk_start: usize,
    cursor: GraphemeCursor,
    next_char_pos: Position,
}

impl<'a> GraphemeIter<'a> {
    pub fn new(buf: &'a RawBuffer, pos: Position) -> GraphemeIter<'a> {
        let slice = buf.rope().slice(..);
        let char_idx = buf.pos_to_char_index(pos);

        // Bounds check
        debug_assert!(char_idx <= slice.len_chars());

        // We work with bytes for this, so convert.
        let byte_idx = slice.char_to_byte(char_idx);

        // Get the chunk with our byte index in it.
        let (chunk, chunk_byte_idx, _, _) = slice.chunk_at_byte(byte_idx);

        // Set up the grapheme cursor.
        let cursor = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

        GraphemeIter {
            rope: slice,
            chunk,
            chunk_start: chunk_byte_idx,
            cursor,
            next_char_pos: pos,
        }
    }

    pub fn next_boundary(&mut self) -> Option<&'a str> {
        let start = self.cursor.cur_cursor();
        loop {
            match self.cursor.next_boundary(self.chunk, self.chunk_start) {
                Ok(None) => return None,
                Ok(Some(end)) => {
                    if start < self.chunk_start {
                        return Some(self.rope.byte_slice(start..end).as_str().unwrap());
                    } else {
                        return Some(
                            &self.chunk[(start - self.chunk_start)..(end - self.chunk_start)],
                        );
                    }
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    self.chunk_start += self.chunk.len();
                    let (a, _, _, _) = self.rope.chunk_at_byte(self.chunk_start);
                    self.chunk = a;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let ctx_chunk = self.rope.chunk_at_byte(n - 1).0;
                    self.cursor.provide_context(ctx_chunk, n - ctx_chunk.len());
                }
                _ => unreachable!(),
            }
        }
    }
}

impl<'a> Iterator for GraphemeIter<'a> {
    type Item = (Position, &'a str);

    /// Returns the next grapheme.
    ///
    /// # Complexity
    ///
    /// Runs in amortized O(K) time and worst-case O(log N + K) time, where K
    /// is the length in bytes of the grapheme.
    fn next(&mut self) -> Option<Self::Item> {
        let grapheme = self.next_boundary()?;
        let pos = self.next_char_pos;
        for ch in grapheme.chars() {
            match ch {
                '\n' => {
                    self.next_char_pos.y += 1;
                    self.next_char_pos.x = 0;
                }
                '\r' => {
                    // Do nothing.
                }
                _ => {
                    self.next_char_pos.x += 1;
                }
            }
        }

        Some((pos, grapheme))
    }
}

#[cfg(test)]
mod tests {
    use crate::raw_buffer::RawBuffer;

    use super::*;

    fn pos(y: usize, x: usize) -> Position {
        Position { y, x }
    }

    fn string(s: &str) -> String {
        s.to_owned()
    }

    #[test]
    fn test_grapheme_iter_next() {
        // ABC
        // XY
        let buffer = RawBuffer::from_text("ABC\nXY");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 0));

        assert_eq!(iter.next(), Some((pos(0, 0), string("A"))));
        assert_eq!(iter.next(), Some((pos(0, 1), string("B"))));
        assert_eq!(iter.next(), Some((pos(0, 2), string("C"))));
        assert_eq!(iter.next(), Some((pos(0, 3), string("\n"))));
        assert_eq!(iter.next(), Some((pos(1, 0), string("X"))));
    }

    #[test]
    fn test_grapheme_iter_prev() {
        // ABC
        // XY
        let buffer = RawBuffer::from_text("ABC\nXY");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(1, 0));

        assert_eq!(iter.prev(), Some((pos(0, 3), string("\n"))));
        assert_eq!(iter.prev(), Some((pos(0, 2), string("C"))));
    }

    #[test]
    fn test_faster_grapheme_iter_next() {
        // ABC
        // XY
        let buffer = RawBuffer::from_text("ABC\nXY");
        let mut iter = buffer.grapheme_iter(Position::new(0, 0));

        assert_eq!(iter.next(), Some((pos(0, 0), "A")));
        assert_eq!(iter.next(), Some((pos(0, 1), "B")));
        assert_eq!(iter.next(), Some((pos(0, 2), "C")));
        assert_eq!(iter.next(), Some((pos(0, 3), "\n")));
        assert_eq!(iter.next(), Some((pos(1, 0), "X")));
    }

    #[test]
    fn test_grapheme_iter_next_with_complicated_emojis() {
        // Note: the emoji is 3-characters wide: U+1F469 U+200D U+1F52C.
        let buffer = RawBuffer::from_text("a👩‍🔬");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 0));

        assert_eq!(iter.next(), Some((pos(0, 0), string("a"))));
        assert_eq!(iter.next(), Some((pos(0, 1), string("👩‍🔬"))));
    }

    #[test]
    fn test_grapheme_iter_prev_with_complicated_emojis() {
        // Note: the emoji is 3-characters wide: U+1F469 U+200D U+1F52C.
        let buffer = RawBuffer::from_text("a👩‍🔬");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 4));

        assert_eq!(iter.prev(), Some((pos(0, 1), string("👩‍🔬"))));
        assert_eq!(iter.prev(), Some((pos(0, 0), string("a"))));
    }

    #[test]
    fn test_fast_grapheme_iter_next_with_complicated_emojis() {
        // Note: the emoji is 3-characters wide: U+1F469 U+200D U+1F52C.
        let buffer = RawBuffer::from_text("a👩‍🔬");
        let mut iter = buffer.grapheme_iter(Position::new(0, 0));

        assert_eq!(iter.next(), Some((pos(0, 0), "a")));
        assert_eq!(iter.next(), Some((pos(0, 1), "👩‍🔬")));
    }

    #[test]
    fn test_grapheme_iter_newline() {
        // ABC
        // XY
        let buffer = RawBuffer::from_text("ABC\nXY");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 3));

        assert_eq!(iter.next(), Some((pos(0, 3), string("\n"))));
        assert_eq!(iter.prev(), Some((pos(0, 3), string("\n"))));
        assert_eq!(iter.next(), Some((pos(0, 3), string("\n"))));
    }

    #[test]
    fn test_grapheme_iter_next2() {
        let buffer = RawBuffer::from_text("ABC");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 0));
        assert_eq!(iter.next().map(|(_pos, s)| s), Some("A".to_string()));
        assert_eq!(iter.next().map(|(_pos, s)| s), Some("B".to_string()));
        assert_eq!(iter.next().map(|(_pos, s)| s), Some("C".to_string()));
        assert_eq!(iter.next().map(|(_pos, s)| s), None);

        let buffer = RawBuffer::from_text("あaい");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 0));
        assert_eq!(iter.next().map(|(_pos, s)| s), Some("あ".to_string()));
        assert_eq!(iter.next().map(|(_pos, s)| s), Some("a".to_string()));
        assert_eq!(iter.next().map(|(_pos, s)| s), Some("い".to_string()));
        assert_eq!(iter.next().map(|(_pos, s)| s), None);

        // A grapheme ("ka" in Katakana with Dakuten), consists of U+304B U+3099.
        let buffer = RawBuffer::from_text("\u{304b}\u{3099}");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 0));
        assert_eq!(
            iter.next().map(|(_pos, s)| s),
            Some("\u{304b}\u{3099}".to_string())
        );
        assert_eq!(iter.next().map(|(_pos, s)| s), None);

        // A grapheme ("u" with some marks), consists of U+0075 U+0308 U+0304.
        let buffer = RawBuffer::from_text("\u{0075}\u{0308}\u{0304}BC");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 0));
        assert_eq!(
            iter.next().map(|(_pos, s)| s),
            Some("\u{0075}\u{0308}\u{0304}".to_string())
        );
        assert_eq!(iter.next().map(|(_pos, s)| s), Some("B".to_string()));
        assert_eq!(iter.next().map(|(_pos, s)| s), Some("C".to_string()));
        assert_eq!(iter.next().map(|(_pos, s)| s), None);
    }

    #[test]
    fn test_grapheme_iter_prev2() {
        let buffer = RawBuffer::from_text("ABC");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 3));
        assert_eq!(iter.prev().map(|(_pos, s)| s), Some("C".to_string()));
        assert_eq!(iter.prev().map(|(_pos, s)| s), Some("B".to_string()));
        assert_eq!(iter.prev().map(|(_pos, s)| s), Some("A".to_string()));
        assert_eq!(iter.prev().map(|(_pos, s)| s), None);

        let buffer = RawBuffer::from_text("あaい");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 3));
        assert_eq!(iter.prev().map(|(_pos, s)| s), Some("い".to_string()));
        assert_eq!(iter.prev().map(|(_pos, s)| s), Some("a".to_string()));
        assert_eq!(iter.prev().map(|(_pos, s)| s), Some("あ".to_string()));
        assert_eq!(iter.prev().map(|(_pos, s)| s), None);

        // A grapheme ("か" with dakuten), consists of U+304B U+3099.
        let buffer = RawBuffer::from_text("\u{304b}\u{3099}");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 2));
        assert_eq!(
            iter.prev().map(|(_pos, s)| s),
            Some("\u{304b}\u{3099}".to_string())
        );
        assert_eq!(iter.prev().map(|(_pos, s)| s), None);

        // A grapheme ("u" with some marks), consists of U+0075 U+0308 U+0304.
        let buffer = RawBuffer::from_text("\u{0075}\u{0308}\u{0304}BC");
        let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 5));
        assert_eq!(iter.prev().map(|(_pos, s)| s), Some("C".to_string()));
        assert_eq!(iter.prev().map(|(_pos, s)| s), Some("B".to_string()));
        assert_eq!(
            iter.prev().map(|(_pos, s)| s),
            Some("\u{0075}\u{0308}\u{0304}".to_string())
        );
        assert_eq!(iter.prev().map(|(_pos, s)| s), None);
    }
}
