use std::cmp::{Ordering, max, min};

use ropey::Rope;

/// The zero-based position in the buffer.
#[derive(PartialEq, Eq, Ord, Copy, Clone)]
pub struct Position {
    line: usize,
    column: usize,
}

impl Position {
    fn new(line: usize, column: usize) -> Self {
        Self { line, column }
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

pub struct Range(Position, Position);

impl Range {
    fn new(start: Position, end: Position) -> Self {
        Self(start, end)
    }

    fn front(&self) -> Position {
        min(self.0, self.1)
    }

    fn back(&self) -> Position {
        max(self.0, self.1)
    }

    /// The position of the cursor after replacing this range of
    /// text with `new_text`.
    fn cursor_after_edit(&self, new_text: &str) -> Position {
        let pos = self.front();
        let num_newlines_added = new_text.matches('\n').count();
        let num_newlines_deleted = self.back().line - self.front().line;

        let y_diff = num_newlines_added.saturating_sub(num_newlines_deleted);

        let mut x_diff = 0;
        for c in new_text.chars() {
            if c == '\n' {
                x_diff = 0;
            } else {
                x_diff += 1;
            }
        }

        let new_y = pos.line + y_diff;
        let new_x = if new_text.contains('\n') {
            x_diff
        } else {
            pos.column + x_diff
        };

        Position::new(new_y, new_x)
    }
}

pub enum Cursor {
    Normal(Position),
    Selection(Range),
}

pub struct Buffer {
    rope: Rope,
    cursors: Vec<Cursor>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            cursors: vec![Cursor::Normal(Position::new(0, 0))],
        }
    }

    pub fn insert_str(&mut self, text: &str) {
        for i in (0..self.cursors.len()).rev() {}
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
