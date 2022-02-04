use crate::{cursor::Position, raw_buffer::RawBuffer};

pub struct CharIter<'a> {
    iter: ropey::iter::Chars<'a>,
    buf: &'a RawBuffer,
    next_pos: Position,
    last_pos: Position,
}

impl<'a> CharIter<'a> {
    pub fn new(
        iter: ropey::iter::Chars<'a>,
        buf: &'a RawBuffer,
        pos: Position,
        prev_was_newline: bool,
    ) -> CharIter<'a> {
        CharIter {
            iter,
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

    pub fn buffer(&self) -> &'a RawBuffer {
        self.buf
    }

    pub(crate) fn reset_internal_state(&mut self) {
        self.iter = self
            .buf
            .rope()
            .chars_at(self.buf.pos_to_rope_index(self.next_pos));
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
                self.next_pos.y -= 1;
                self.next_pos.x = self.buf.line_len(self.next_pos.y);
            }
            Some('\r') => {
                // Do nothing.
            }
            Some(_) => {
                self.next_pos.x = self.next_pos.x.saturating_sub(1);
            }
            None => {
                // Do nothing.
            }
        }
        self.last_pos = self.next_pos;
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
        self.last_pos = self.next_pos;
        match ch {
            Some('\n') => {
                self.next_pos.y += 1;
                self.next_pos.x = 0;
            }
            Some('\r') => {
                // Do nothing.
            }
            Some(_) => {
                self.next_pos.x += 1;
            }
            None => {
                // Do nothing.
            }
        }
        ch
    }
}

impl Clone for CharIter<'_> {
    fn clone(&self) -> Self {
        // Create ropey::iter::Chars again to reset it's internal state
        // `last_call_was_prev_impl`.
        self.buf.char_iter(self.next_pos)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_char_iter() {
        let buffer = RawBuffer::from_text("ABC");
        let mut iter = buffer.char_iter(Position::new(0, 1));
        assert_eq!(iter.last_position(), Position::new(0, 1));
        assert_eq!(iter.next(), Some('B'));
        assert_eq!(iter.last_position(), Position::new(0, 1));
        assert_eq!(iter.next(), Some('C'));
        assert_eq!(iter.last_position(), Position::new(0, 2));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.last_position(), Position::new(0, 3));
        assert_eq!(iter.prev(), Some('C'));
        assert_eq!(iter.last_position(), Position::new(0, 2));
        assert_eq!(iter.prev(), Some('B'));
        assert_eq!(iter.last_position(), Position::new(0, 1));
        assert_eq!(iter.prev(), Some('A'));
        assert_eq!(iter.last_position(), Position::new(0, 0));
        assert_eq!(iter.next(), Some('A'));
        assert_eq!(iter.last_position(), Position::new(0, 0));
        assert_eq!(iter.next(), Some('B'));
        assert_eq!(iter.last_position(), Position::new(0, 1));
    }

    #[test]
    fn newline() {
        let buffer = RawBuffer::from_text("A\nB");
        let mut iter = buffer.char_iter(Position::new(0, 0));
        assert_eq!(iter.next(), Some('A'));
        assert_eq!(iter.last_position(), Position::new(0, 0));
        assert_eq!(iter.next(), Some('\n'));
        assert_eq!(iter.last_position(), Position::new(0, 1));
        assert_eq!(iter.next(), Some('B'));
        assert_eq!(iter.last_position(), Position::new(1, 0));
        assert_eq!(iter.prev(), Some('B'));
        assert_eq!(iter.last_position(), Position::new(1, 0));
        assert_eq!(iter.prev(), Some('\n'));
        assert_eq!(iter.last_position(), Position::new(0, 1));
        assert_eq!(iter.prev(), Some('A'));
        assert_eq!(iter.last_position(), Position::new(0, 0));
    }
}
