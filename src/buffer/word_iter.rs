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
}

impl<'a> WordIter<'a> {
    pub fn new(iter: CharIter<'a>) -> WordIter<'a> {
        WordIter { iter }
    }

    pub fn new_from_current_word(mut iter: CharIter<'a>) -> WordIter<'a> {
        while let Some(ch) = iter.prev() {
            if !is_word_char(ch) {
                break;
            }
        }

        // Clone to reset last_call_was_prev_impl.
        WordIter { iter: iter.clone() }
    }

    pub fn position(&self) -> Position {
        self.iter.last_position()
    }

    pub fn prev(&mut self) -> Option<Word<'_>> {
        // TODO:
        None
    }
}

impl<'a> Iterator for WordIter<'a> {
    type Item = Word<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut end = self.iter.clone();

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

        while let Some(ch) = start.prev() {
            if !is_word_char(ch) {
                start.next();
                break;
            }
        }

        // Find the end of the word.
        while let Some(ch) = end.next() {
            if !is_word_char(ch) {
                break;
            }
        }

        // Clone to reset last_call_was_prev_impl.
        self.iter = end.clone();

        Some(Word {
            buf: self.iter.buffer(),
            range: Range::from_positions(start.next_position(), end.last_position()),
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
    fn word_iter_from_current_word() {
        let buffer = RawBuffer::from_text("");
        let mut iter = buffer.word_iter_from_current_word(Position::new(0, 0));
        assert_eq!(next_word(&mut iter), None);

        let buffer = RawBuffer::from_text("ABC DEF XYZ");
        let mut iter = buffer.word_iter_from_current_word(Position::new(0, 1));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 0, 0, 3)));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 4, 0, 7)));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 8, 0, 11)));
        assert_eq!(next_word(&mut iter), None);

        let mut iter = buffer.word_iter_from_current_word(Position::new(0, 3));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 0, 0, 3)));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 4, 0, 7)));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 8, 0, 11)));
        assert_eq!(next_word(&mut iter), None);

        let buffer = RawBuffer::from_text("    foo(bar, baz)");
        let mut iter = buffer.word_iter_from_current_word(Position::new(0, 0));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 4, 0, 7))); // "foo"
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 8, 0, 11))); // "bar"
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 13, 0, 16))); // "baz"
        assert_eq!(next_word(&mut iter), None);

        let buffer = RawBuffer::from_text("ABC\nUVW XYZ");
        let mut iter = buffer.word_iter_from_current_word(Position::new(0, 0));
        assert_eq!(next_word(&mut iter), Some(Range::new(0, 0, 0, 3)));
        assert_eq!(next_word(&mut iter), Some(Range::new(1, 0, 1, 3)));
        assert_eq!(next_word(&mut iter), Some(Range::new(1, 4, 1, 7)));
        assert_eq!(next_word(&mut iter), None);

        let mut iter = buffer.word_iter_from_current_word(Position::new(1, 0));
        assert_eq!(next_word(&mut iter), Some(Range::new(1, 0, 1, 3)));
        assert_eq!(next_word(&mut iter), Some(Range::new(1, 4, 1, 7)));
        assert_eq!(next_word(&mut iter), None);
    }
}
