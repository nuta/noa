use std::cmp::{Ordering, max, min};

use ropey::Rope;

/// The zero-based position in the buffer.
#[derive(PartialEq, Eq, Ord, Copy, Clone)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    fn rope_index(&self, rope: &Rope) -> usize {
        rope.line_to_char(self.line) + self.column
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.line < other.line {
            Some(Ordering::Less)
        } else if self.line > other.line {
            Some(Ordering::Greater)
        } else {
            self.column.partial_cmp(&other.column)
        }
    }
}

pub struct Range {
    anchor: Position,
    head: Position,
}

impl Range {
    fn new(anchor: Position, head: Position) -> Self {
        Self { anchor, head }
    }

    fn new_at(pos: Position) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn front(&self) -> Position {
        min(self.anchor, self.head)
    }

    pub fn back(&self) -> Position {
        max(self.anchor, self.head)
    }

    fn rope_range(&self, rope: &Rope) -> std::ops::Range<usize> {
        let front = self.front().rope_index(rope);
        let back = self.back().rope_index(rope);
        front..back
    }
}

pub struct Cursor {
    pub range: Range,
}

impl Cursor {
    fn new(range: Range) -> Self {
        Self { range }
    }

    /// Move the cursor after replacing this range of text with `new_text`.
    fn move_after_edit(&mut self, new_text: &str) {
        let front = self.range.front();
        let num_newlines_added = new_text.matches('\n').count();
        let num_newlines_deleted = self.range.back().line - front.line;

        let y_diff = num_newlines_added.saturating_sub(num_newlines_deleted);

        let mut x_diff = 0;
        for c in new_text.chars() {
            if c == '\n' {
                x_diff = 0;
            } else {
                x_diff += 1;
            }
        }

        let new_y = front.line + y_diff;
        let new_x = if new_text.contains('\n') {
            x_diff
        } else {
            front.column + x_diff
        };

        let new_pos = Position::new(new_y, new_x);
        self.range = Range::new_at(new_pos);
    }
}

pub struct Buffer {
    rope: Rope,
    cursors: Vec<Cursor>,
}

impl Buffer {
    pub fn new() -> Self {
        Self::from_rope(Rope::new())
    }

    pub fn from_rope(rope: Rope) -> Self {
        Self {
            rope,
            cursors: vec![Cursor::new(Range::new_at(Position::new(0, 0)))],
        }
    }

    pub fn from_reader(reader: impl std::io::Read) -> Result<Self, std::io::Error> {
        let rope = Rope::from_reader(reader)?;
        Ok(Self::from_rope(rope))
    }

    pub fn num_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn insert_str(&mut self, text: &str) {
        for c in self.cursors.iter_mut().rev() {
            let rope_range = c.range.rope_range(&self.rope);
            self.rope.remove(rope_range.clone());
            self.rope.insert(rope_range.start, text);
            c.move_after_edit(text);
        }
    }
}

impl ToString for Buffer {
    fn to_string(&self) -> String {
        self.rope.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_str() {
        let mut buffer = Buffer::new();
        buffer.insert_str("Hello, world!");
        assert_eq!(buffer.to_string(), "Hello, world!");
    }
}
