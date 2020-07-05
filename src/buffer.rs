use std::fmt;
use ropey::Rope;

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

    pub fn index_in_rope(&self, rope: &Rope) -> usize {
        rope.line_to_char(self.line) + self.col
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

#[derive(Debug, PartialEq, Clone)]
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

    pub fn cursors(&self) -> &[Cursor] {
        &self.cursors
    }

    pub fn move_by_insertion(&mut self, string: &str) -> Vec<Cursor> {
        let y_diff = string.matches('\n').count();
        let x_diff = string.rfind('\n').unwrap_or(string.len());

        let old_cursors = self.cursors.clone();
        for cursor in &mut self.cursors {
            let new_pos = match cursor {
                Cursor::Normal(Point { line, col }) => {
                    Point::new(*line + y_diff, *col + x_diff)
                }
                Cursor::Selection(Range { start, .. }) => {
                    Point::new(start.line + y_diff, start.col + x_diff)
                }
            };

            // TODO: Adjust other cursors.
            *cursor = Cursor::Normal(new_pos);
        }

        old_cursors
    }
}

pub struct Buffer {
    buf: Rope,
    cursors: CursorSet,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            buf: Rope::new(),
            cursors: CursorSet::new(),
        }
    }

    pub fn text(&self) -> String {
        self.buf.to_string()
    }

    pub fn insert(&mut self, string: &str) {
        for cursor in self.cursors.move_by_insertion(&string) {
            match cursor {
                Cursor::Normal(pos) => {
                    self.insert_range(string, &pos);
                }
                Cursor::Selection(range) => {
                    self.delete_range(&range);
                    self.insert_range(string, &range.start);
                }
            };
        }
    }

    pub fn begin_undo_group(&mut self) {
    }

    pub fn end_undo_group(&mut self) {
    }

    pub fn undo(&mut self) {
    }

    pub fn redo(&mut self) {
    }

    fn insert_range(&mut self, string: &str, pos: &Point) {
        self.buf.insert(pos.index_in_rope(&self.buf), string);
    }

    fn delete_range(&mut self, range: &Range) {
        let start = range.start.index_in_rope(&self.buf);
        let end = range.end.index_in_rope(&self.buf);
        self.buf.remove(start..end);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn single_cursor() {
        let mut b = Buffer::new();
        b.insert("Hello");
        b.insert(" World!");
        assert_eq!(b.text(), "Hello World!");
    }
}
