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

    pub fn position(&self) -> Position {
        self.iter.position()
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

        assert_eq!(iter.position(), Position::new(0, 0));
        assert_eq!(iter.next(), Some(ArrayString::from_str("A").unwrap()));
        assert_eq!(iter.position(), Position::new(0, 1));
        assert_eq!(iter.next(), Some(ArrayString::from_str("B").unwrap()));
        assert_eq!(iter.position(), Position::new(0, 2));
        assert_eq!(iter.next(), Some(ArrayString::from_str("C").unwrap()));
        assert_eq!(iter.position(), Position::new(0, 3));
        assert_eq!(iter.next(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.position(), Position::new(1, 0));
        assert_eq!(iter.next(), Some(ArrayString::from_str("X").unwrap()));
        assert_eq!(iter.position(), Position::new(1, 0));
    }

    #[test]
    fn test_grapheme_iter_prev() {
        // ABC
        // XY
        let buffer = RawBuffer::from_text("ABC\nXY");
        let mut iter = buffer.grapheme_iter(Position::new(1, 0));

        assert_eq!(iter.position(), Position::new(1, 0));
        assert_eq!(iter.prev(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.position(), Position::new(0, 3));
        assert_eq!(iter.prev(), Some(ArrayString::from_str("C").unwrap()));
        assert_eq!(iter.position(), Position::new(0, 2));
    }

    #[test]
    fn test_grapheme_iter_newline() {
        // ABC
        // XY
        let buffer = RawBuffer::from_text("ABC\nXY");
        let mut iter = buffer.grapheme_iter(Position::new(0, 3));

        assert_eq!(iter.position(), Position::new(0, 3));
        assert_eq!(iter.next(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.position(), Position::new(1, 0));
        assert_eq!(iter.prev(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.position(), Position::new(0, 3));
        assert_eq!(iter.next(), Some(ArrayString::from_str("\n").unwrap()));
        assert_eq!(iter.position(), Position::new(1, 0));
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
