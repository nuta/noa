use std::fmt;
use crate::gap_buffer::GapBuffer;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Point {
    pub line: usize,
    pub col: usize,
}

impl Point {
    pub fn new(line: usize, col: usize) -> Point {
        Point {
            line,
            col,
        }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.line, self.col)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Range {
    pub start: Point,
    pub end: Point,
}

impl Range {
    pub fn new(start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Range {
        Range::from_points(
            Point::new(start_line, start_col),
            Point::new(end_line, end_col)
        )
    }

    pub fn from_points(start: Point, end: Point) -> Range {
        Range {
            start,
            end,
        }
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end)
    }
}

#[derive(Debug, PartialEq)]
enum Cursor {
    Normal(Point),
    Selection(Range),
}

struct CursorSet {
    cursors: Vec<Cursor>,
}

impl CursorSet {
    pub fn new() -> CursorSet {
        CursorSet {
            cursors: vec![Cursor::Normal(Point::new(0, 0))],
        }
    }
}

pub struct Buffer {
    gb: GapBuffer,
    cursors: CursorSet,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            gb: GapBuffer::new(),
            cursors: CursorSet::new(),
        }
    }

    pub fn insert_range(string: &str, range: &Range) {
    }

    pub fn delete_range(range: &Range) {
    }

    pub fn begin_undo_group(&mut self) {
    }

    pub fn end_undo_group(&mut self) {
    }

    pub fn undo(&mut self) {
    }

    pub fn redo(&mut self) {
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn single_cursor() {
        let mut b = Buffer::new();
    }
}
