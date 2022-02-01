use crate::{cursor::Position, raw_buffer::RawBuffer};

#[derive(Clone)]
pub struct CharIter<'a> {
    iter: ropey::iter::Chars<'a>,
    buf: &'a RawBuffer,
    pos: Position,
    prev_was_newline: bool,
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
            pos,
            prev_was_newline,
        }
    }

    pub fn position(&self) -> Position {
        self.pos
    }

    pub fn buffer(&self) -> &'a RawBuffer {
        self.buf
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
                self.pos.x = self.pos.x.saturating_sub(1);
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
                self.prev_was_newline = true;
            }
            Some('\r') => {
                // Do nothing.
            }
            Some(_) if self.prev_was_newline => {
                // Don't advance the cursor if the previous character was a newline.
                self.prev_was_newline = false;
            }
            Some(_) => {
                dbg!(ch.unwrap(), self.pos);
                self.pos.x += 1;
            }
            None => {
                // Do nothing.
            }
        }
        ch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_iter() {
        let buffer = RawBuffer::from_text("XY\n123");
        let mut iter = buffer.char_iter(Position::new(1, 1));
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
    }

    #[test]
    fn newline() {
        // let buffer = RawBuffer::from_text("A\nB");
        // let mut iter = buffer.char_iter(Position::new(0, 0));
        // assert_eq!(iter.next(), Some('A'));
        // assert_eq!(iter.position(), Position::new(0, 0));
        // assert_eq!(iter.next(), Some('\n'));
        // assert_eq!(iter.position(), Position::new(0, 1));
        // assert_eq!(iter.next(), Some('B'));
        // assert_eq!(iter.position(), Position::new(1, 0));
        // assert_eq!(iter.prev(), Some('B'));
        // assert_eq!(iter.position(), Position::new(1, 0));
        // assert_eq!(iter.prev(), Some('\n'));
        // assert_eq!(iter.position(), Position::new(0, 1));
        // assert_eq!(iter.prev(), Some('A'));
        // assert_eq!(iter.position(), Position::new(1, 0));
    }
}
