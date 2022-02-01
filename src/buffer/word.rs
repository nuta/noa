use crate::{
    char_iter::CharIter,
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
};

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
    range: Option<Range>,
}

impl<'a> WordIter<'a> {
    pub fn new(iter: CharIter<'a>, range: Option<Range>) -> WordIter<'a> {
        WordIter { iter, range }
    }

    pub fn position(&self) -> Position {
        self.iter.position()
    }

    pub fn range(&self) -> Option<&Range> {
        self.range.as_ref()
    }

    pub fn prev(&mut self) -> Option<Word<'_>> {
        // TODO:
        None
    }
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
            buf: self.iter.buffer(),
            range: Range::from_positions(start.position(), end.position()),
        })
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_word() {
        let buffer = RawBuffer::from_text("");
        let mut iter = buffer.word_iter(Position::new(0, 0));
        assert_eq!(iter.next().map(|w| w.range()), None);

        let buffer = RawBuffer::from_text("A");
        let mut iter = buffer.word_iter(Position::new(0, 0));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 0, 0, 1)));
        assert_eq!(iter.next().map(|w| w.range()), None);

        let buffer = RawBuffer::from_text("ABC DEF");
        let mut iter = buffer.word_iter(Position::new(0, 3));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 0, 0, 3)));

        let buffer = RawBuffer::from_text("abc WXYZ   12");
        let mut iter = buffer.word_iter(Position::new(0, 0));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 0, 0, 3)));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 4, 0, 8)));
        assert_eq!(
            iter.next().map(|w| w.range()),
            Some(Range::new(0, 11, 0, 13))
        );
        assert_eq!(iter.next().map(|w| w.range()), None);

        let mut iter = buffer.word_iter(Position::new(0, 5));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 4, 0, 8)));

        let mut iter = buffer.word_iter(Position::new(0, 8));
        assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 4, 0, 8)));
    }
}
