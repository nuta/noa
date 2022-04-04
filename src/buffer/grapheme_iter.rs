use arrayvec::ArrayString;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use crate::{char_iter::CharIter, cursor::Position};

#[derive(Clone)]
pub struct GraphemeIter<'a> {
    iter: CharIter<'a>,
}

impl<'a> GraphemeIter<'a> {
    pub fn new(iter: CharIter<'a>) -> GraphemeIter<'a> {
        GraphemeIter { iter }
    }

    pub fn next_position(&self) -> Position {
        self.iter.next_position()
    }

    pub fn last_position(&self) -> Position {
        self.iter.last_position()
    }

    /// Returns the previous grapheme.
    ///
    /// # Complexity
    ///
    /// Runs in amortized O(K) time and worst-case O(log N + K) time, where K
    /// is the length in bytes of the grapheme.
    pub fn prev(&mut self) -> Option<ArrayString<16>> {
        // Must be large enough to hold all characters in a grapheme.
        const OFFSET_END: usize = 16;

        let mut cursor = GraphemeCursor::new(
            OFFSET_END, 0, /* AFAIK this field is used in prev_boundary */
            true,
        );
        let mut chunk = String::new();
        let mut char_start = self.iter.clone();
        loop {
            match self.iter.prev() {
                Some(ch) => {
                    chunk.insert(0, ch);
                }
                None => {
                    // Reached to the EOF.
                    if chunk.is_empty() {
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

            match cursor.prev_boundary(&chunk, OFFSET_END - chunk.len()) {
                Ok(Some(offset)) => {
                    // Characters comes in reverse order "CBA".
                    let mut grapheme = ArrayString::new();
                    for ch in chunk[(chunk.len() - (OFFSET_END - offset))..].chars().rev() {
                        grapheme.push(ch);
                        char_start.prev();
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
                Err(GraphemeIncomplete::PreContext(mut n)) => {
                    let mut new_chunk = String::new();
                    let mut iter = self.iter.clone();
                    while n > 0 {
                        if let Some(ch) = iter.prev() {
                            new_chunk.insert(0, ch);
                            n -= n.saturating_sub(ch.len_utf8());
                        } else {
                            break;
                        }
                    }
                    cursor.provide_context(
                        &new_chunk[..],
                        OFFSET_END - chunk.len() - new_chunk.len(),
                    );
                }
            }
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
        // Not sure if `std::usize::MAX` cause problems.
        let mut cursor = GraphemeCursor::new(0, std::usize::MAX, true);
        let mut char_start = self.iter.clone();
        let mut chunk = String::new();
        loop {
            match self.iter.next() {
                Some(ch) => {
                    chunk.push(ch);
                }
                None => {
                    // Reached to the EOF.
                    if chunk.is_empty() {
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

            match cursor.next_boundary(&chunk, 0) {
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
                    // Continue his loop.
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
                    // Here's unreachable because `chunk` contains a complete grapheme.
                    unreachable!();
                }
            }
        }
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

        // FIXME: It should return the whole emoji, not a part of it.
        assert_eq!(iter.next_position(), Position::new(0, 4));
        assert_eq!(iter.prev(), Some(ArrayString::from_str("🔬").unwrap()));
        assert_eq!(iter.next_position(), Position::new(0, 3));
        assert_eq!(
            iter.prev(),
            Some(ArrayString::from_str("\u{200d}👩").unwrap())
        );
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
