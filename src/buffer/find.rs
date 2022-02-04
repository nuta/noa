use crate::{char_iter::CharIter, cursor::Position};

pub struct FindIter<'a, 'b> {
    chars: CharIter<'a>,
    query: &'b str,
}

impl<'a, 'b> FindIter<'a, 'b> {
    pub fn new(chars: CharIter<'a>, query: &'b str) -> FindIter<'a, 'b> {
        FindIter { chars, query }
    }
}

impl<'a, 'b> Iterator for FindIter<'a, 'b> {
    type Item = Position;

    fn next(&mut self) -> Option<Position> {
        let mut query_iter = self.query.chars();
        let mut buf_iter = self.chars.clone();
        let pos = buf_iter.last_position();

        self.chars.next();

        loop {
            match (buf_iter.next(), query_iter.next()) {
                (Some(a), Some(b)) if a != b => {
                    return None;
                }
                (None, Some(_)) => {
                    // Reached to EOF.
                    return None;
                }
                (_, None) => {
                    return Some(pos);
                }
                (Some(_), Some(_)) => {
                    // Continue comparing the next characters...
                }
            }
        }
    }
}

impl<'a, 'b> DoubleEndedIterator for FindIter<'a, 'b> {
    fn next_back(&mut self) -> Option<Position> {
        let mut query_iter = self.query.chars();
        let mut buf_iter = self.chars.clone();
        let pos = buf_iter.last_position();

        self.chars.prev();

        loop {
            match (buf_iter.next(), query_iter.next()) {
                (Some(a), Some(b)) if a != b => {
                    return None;
                }
                (None, Some(_)) => {
                    // Reached to EOF.
                    return None;
                }
                (_, None) => {
                    return Some(pos);
                }
                (Some(_), Some(_)) => {
                    // Continue comparing the next characters...
                }
            }
        }
    }
}
