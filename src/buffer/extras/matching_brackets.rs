use crate::{buffer::Buffer, cursor::Position};

impl Buffer {
    pub fn matching_bracket(&mut self, pos: Position) -> Option<Position> {
        let mut char_iter = self.char_iter(pos);
        let (opening, correspond_ch) = loop {
            match char_iter.next() {
                Some('(') => break (true, ')'),
                Some('{') => break (true, '}'),
                Some('[') => break (true, ']'),
                Some('<') => break (true, '>'),
                Some(')') => break (false, '('),
                Some('}') => break (false, '{'),
                Some(']') => break (false, '['),
                Some('>') => break (false, '<'),
                Some(_) => continue,
                None => {
                    return None;
                }
            }
        };

        while let Some(ch) = if opening {
            char_iter.next()
        } else {
            char_iter.prev()
        } {
            dbg!(ch);
            if ch == correspond_ch {
                return Some(char_iter.last_position());
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn matching_bracket() {
        let mut b = Buffer::from_text("");
        assert_eq!(b.matching_bracket(Position::new(0, 0)), None);

        let mut b = Buffer::from_text("{}");
        assert_eq!(
            b.matching_bracket(Position::new(0, 0)),
            Some(Position::new(0, 1))
        );
        assert_eq!(
            b.matching_bracket(Position::new(0, 1)),
            Some(Position::new(0, 0))
        );
    }
}
