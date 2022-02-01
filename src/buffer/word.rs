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

        // Skip until the start of the next word.
        loop {
            match end.next() {
                Some(ch) if is_word_char(ch) => {
                    break;
                }
                Some(_) => {
                    continue;
                }
                None => {
                    return None;
                }
            }
        }

        // Find the beginning of the word.
        let mut start = end.clone();
        dbg!("start");
        while let Some(ch) = start.prev() {
            if !is_word_char(ch) {
                start.next();
                break;
            }
        }

        // Find the end of the word.
        dbg!("end");
        while let Some(ch) = end.next() {
            if !is_word_char(ch) {
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

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn next_word(iter: &mut WordIter) -> Option<Range> {
        iter.next().map(|w| w.range())
    }

    #[test]
    fn test_word() {
        let buffer = RawBuffer::from_text("");
        let mut iter = buffer.word_iter(Position::new(0, 0));
        assert_eq!(next_word(&mut iter), None);

        let buffer = RawBuffer::from_text("ABC DEF XYZ");
        let mut iter = buffer.word_iter(Position::new(0, 0));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 0, 0, 3)));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 4, 0, 7)));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 8, 0, 11)));
        assert_eq!(next_word(&mut iter), None);

        let buffer = RawBuffer::from_text("ABC\nXYZ");
        let mut iter = buffer.word_iter(Position::new(0, 0));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 0, 0, 3)));
        assert_eq!(next_word(&mut iter), Some(Range::new(1, 0, 1, 3)));
        assert_eq!(next_word(&mut iter), None);
    }

    #[test]
    fn test_word_iter_around_newline() {
        // let buffer = RawBuffer::from_text("ABC\nDEF");
        // let mut iter = buffer.word_iter(Position::new(0, 0));
        // assert_eq!(next_word(&mut iter), Some(Range::new(0, 0, 0, 3)));
    }
}
