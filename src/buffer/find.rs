use crate::{char_iter::CharIter, cursor::Position};

pub struct FindIter<'a, 'b> {
    chars: CharIter<'a>,
    query: &'b str,
}

impl<'a, 'b> FindIter<'a, 'b> {
    pub fn new(chars: CharIter<'a>, query: &'b str) -> FindIter<'a, 'b> {
        FindIter { chars, query }
    }

    pub fn prev(&mut self) -> Option<Position> {
        if self.query.is_empty() {
            return None;
        }

        loop {
            let mut query_iter = self.query.chars().rev();
            let mut buf_iter = self.chars.clone();

            self.chars.prev();

            let mut n = 0;
            loop {
                let pos = buf_iter.last_position();
                match (buf_iter.prev(), query_iter.next()) {
                    (Some(a), Some(b)) if a != b => {
                        break;
                    }
                    (None, Some(_)) => {
                        // Reached to EOF.
                        return None;
                    }
                    (_, None) => {
                        for _ in 0..n - 1 {
                            self.chars.prev();
                        }

                        return Some(pos);
                    }
                    (Some(_), Some(_)) => {
                        // Continue comparing the next characters...
                        n += 1;
                    }
                }
            }
        }
    }
}

impl<'a, 'b> Iterator for FindIter<'a, 'b> {
    type Item = Position;

    fn next(&mut self) -> Option<Position> {
        if self.query.is_empty() {
            return None;
        }

        loop {
            let mut query_iter = self.query.chars();
            let mut buf_iter = self.chars.clone();
            let pos = buf_iter.next_position();

            self.chars.next();

            let mut n = 0;
            loop {
                match (buf_iter.next(), query_iter.next()) {
                    (Some(a), Some(b)) if a != b => {
                        break;
                    }
                    (None, Some(_)) => {
                        // Reached to EOF.
                        return None;
                    }
                    (_, None) => {
                        for _ in 0..n - 1 {
                            self.chars.next();
                        }

                        return Some(pos);
                    }
                    (Some(_), Some(_)) => {
                        // Continue comparing the next characters...
                        n += 1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::buffer::Buffer;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_find_next() {
        let b = Buffer::from_text("");
        let mut iter = b.find_iter("A", Position::new(0, 0));
        assert_eq!(iter.next(), None);

        let b = Buffer::from_text("AAAA");
        let mut iter = b.find_iter("", Position::new(0, 0));
        assert_eq!(iter.next(), None);
        let mut iter = b.find_iter("B", Position::new(0, 0));
        assert_eq!(iter.next(), None);
        let mut iter = b.find_iter("A", Position::new(0, 0));
        assert_eq!(iter.next(), Some(Position::new(0, 0)));
        assert_eq!(iter.next(), Some(Position::new(0, 1)));
        assert_eq!(iter.next(), Some(Position::new(0, 2)));
        assert_eq!(iter.next(), Some(Position::new(0, 3)));
        assert_eq!(iter.next(), None);
        let mut iter = b.find_iter("A", Position::new(0, 2));
        assert_eq!(iter.next(), Some(Position::new(0, 2)));
        assert_eq!(iter.next(), Some(Position::new(0, 3)));
        assert_eq!(iter.next(), None);
        let mut iter = b.find_iter("AA", Position::new(0, 0));
        assert_eq!(iter.next(), Some(Position::new(0, 0)));
        assert_eq!(iter.next(), Some(Position::new(0, 2)));
        assert_eq!(iter.next(), None);

        let b = Buffer::from_text("AxAxA");
        let mut iter = b.find_iter("A", Position::new(0, 0));
        assert_eq!(iter.next(), Some(Position::new(0, 0)));
        assert_eq!(iter.next(), Some(Position::new(0, 2)));
        assert_eq!(iter.next(), Some(Position::new(0, 4)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_find_prev() {
        let b = Buffer::from_text("");
        let mut iter = b.find_iter("A", Position::new(0, 0));
        assert_eq!(iter.prev(), None);

        let b = Buffer::from_text("AAAA");
        let mut iter = b.find_iter("", Position::new(0, 4));
        assert_eq!(iter.prev(), None);
        let mut iter = b.find_iter("B", Position::new(0, 4));
        assert_eq!(iter.prev(), None);
        let mut iter = b.find_iter("A", Position::new(0, 4));
        assert_eq!(iter.prev(), Some(Position::new(0, 3)));
        assert_eq!(iter.prev(), Some(Position::new(0, 2)));
        assert_eq!(iter.prev(), Some(Position::new(0, 1)));
        assert_eq!(iter.prev(), Some(Position::new(0, 0)));
        assert_eq!(iter.prev(), None);
        let mut iter = b.find_iter("A", Position::new(0, 2));
        assert_eq!(iter.prev(), Some(Position::new(0, 1)));
        assert_eq!(iter.prev(), Some(Position::new(0, 0)));
        assert_eq!(iter.prev(), None);
        let mut iter = b.find_iter("AA", Position::new(0, 4));
        assert_eq!(iter.prev(), Some(Position::new(0, 2)));
        assert_eq!(iter.prev(), Some(Position::new(0, 0)));
        assert_eq!(iter.prev(), None);

        let b = Buffer::from_text("AxAxA");
        let mut iter = b.find_iter("A", Position::new(0, 5));
        assert_eq!(iter.prev(), Some(Position::new(0, 4)));
        assert_eq!(iter.prev(), Some(Position::new(0, 2)));
        assert_eq!(iter.prev(), Some(Position::new(0, 0)));
        assert_eq!(iter.prev(), None);
    }
}
