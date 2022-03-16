use crate::{
    buffer::Buffer,
    char_iter::CharIter,
    cursor::{Position, Range},
};

impl Buffer {
    pub fn matching_bracket(&mut self, pos: Position) -> Option<Range> {
        let (mut char_iter, opening, start_ch, end_ch) = self.find_bracket_nearby(pos)?;
        debug_assert_ne!(start_ch, end_ch);

        let start_pos = char_iter.last_position();
        let mut nested = 0;
        while let Some(ch) = if opening {
            char_iter.next()
        } else {
            char_iter.prev()
        } {
            if start_pos == char_iter.last_position() {
                continue;
            }

            if ch == start_ch {
                nested += 1;
            }

            if ch == end_ch {
                if nested == 0 {
                    let start = char_iter.last_position();
                    let end = Position::new(start.y, start.x + 1);
                    return Some(Range::from_positions(start, end));
                } else {
                    nested -= 1;
                }
            }
        }

        None
    }

    pub fn find_bracket_nearby(&mut self, pos: Position) -> Option<(CharIter, bool, char, char)> {
        let get_corresponding_char = |c: char| match c {
            '(' => Some((true, ')')),
            '{' => Some((true, '}')),
            '[' => Some((true, ']')),
            '<' => Some((true, '>')),
            ')' => Some((false, '(')),
            '}' => Some((false, '{')),
            ']' => Some((false, '[')),
            '>' => Some((false, '<')),
            _ => None,
        };

        // Try the next character.
        let mut char_iter = self.char_iter(pos);
        if let Some(c) = char_iter.next() {
            if let Some((opening, correspond_ch)) = get_corresponding_char(c) {
                return Some((char_iter, opening, c, correspond_ch));
            }
        }

        // Try the previous character.
        let mut char_iter = self.char_iter(pos);
        if let Some(c) = char_iter.prev() {
            if let Some((opening, correspond_ch)) = get_corresponding_char(c) {
                return Some((char_iter, opening, c, correspond_ch));
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
            Some(Range::new(0, 1, 0, 2))
        );
        assert_eq!(
            b.matching_bracket(Position::new(0, 1)),
            Some(Range::new(0, 0, 0, 1))
        );

        let mut b = Buffer::from_text("{{{}}}");
        assert_eq!(
            b.matching_bracket(Position::new(0, 0)),
            Some(Range::new(0, 5, 0, 6))
        );
        assert_eq!(
            b.matching_bracket(Position::new(0, 1)),
            Some(Range::new(0, 4, 0, 5))
        );

        let mut b = Buffer::from_text("{abc}");
        assert_eq!(
            b.matching_bracket(Position::new(0, 0)),
            Some(Range::new(0, 4, 0, 5))
        );
        assert_eq!(
            b.matching_bracket(Position::new(0, 1)),
            Some(Range::new(0, 4, 0, 5))
        );
        assert_eq!(
            b.matching_bracket(Position::new(0, 4)),
            Some(Range::new(0, 0, 0, 1))
        );
        assert_eq!(
            b.matching_bracket(Position::new(0, 5)),
            Some(Range::new(0, 0, 0, 1))
        );
    }
}
