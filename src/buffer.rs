use std::fmt;
use std::collections::HashSet;
use ropey::Rope;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Point {
    pub y: usize,
    pub x: usize,
}

impl Point {
    pub fn new(y: usize, x: usize) -> Point {
        Point {
            y,
            x,
        }
    }

    pub fn index_in_rope(&self, rope: &Rope) -> usize {
        rope.line_to_char(self.y) + self.x
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.y, self.x)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Range {
    pub start: Point,
    pub end: Point,
}

impl Range {
    pub fn new(start_y: usize, start_x: usize, end_y: usize, end_x: usize) -> Range {
        Range::from_points(Point::new(start_y, start_x), Point::new(end_y, end_x))
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

        let cursors = self.cursors.clone();
        let mut lines_changed = HashSet::new();
        for cursor in &mut self.cursors {
            let insert_at = match cursor {
                Cursor::Normal(pos) => {
                    pos
                }
                Cursor::Selection(Range { start, .. }) => {
                    start
                }
            };

            // TODO: Adjust other cursors.
            let x = if string.contains('\n') {
                x_diff
            } else {
                insert_at.x + x_diff
            };

            let new_pos = Point::new(insert_at.y + y_diff, x);
            *cursor = Cursor::Normal(new_pos);
            if lines_changed.contains(&new_pos.y) {
            }

            lines_changed.insert(new_pos.y);
        }

        cursors
    }
}

pub struct Buffer {
    buf: Rope,
    cursors: CursorSet,
    undo_stack: Vec<Rope>,
    redo_stack: Vec<Rope>,
}

impl Buffer {
    pub fn new() -> Buffer {
        let mut buffer = Buffer {
            buf: Rope::new(),
            cursors: CursorSet::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };

        buffer.mark_undo_point();
        buffer
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

    pub fn mark_undo_point(&mut self) {
        self.undo_stack.push(self.buf.clone());
    }

    pub fn undo(&mut self) {
        if let Some(top) = self.undo_stack.last() {
            if *top == self.buf {
                self.undo_stack.pop();
            }
        }

        if let Some(buf) = self.undo_stack.pop() {
            self.redo_stack.push(self.buf.clone());
            self.buf = buf.clone();
        }
    }

    pub fn redo(&mut self) {
        if let Some(top) = self.redo_stack.last() {
            if *top == self.buf {
                self.redo_stack.pop();
            }
        }

        if let Some(buf) = self.redo_stack.pop() {
            self.undo_stack.push(self.buf.clone());
            self.buf = buf.clone();
        }
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

    #[test]
    fn undo() {
        let mut b = Buffer::new();
        b.undo();
        b.redo();
        assert_eq!(b.text(), "");
        b.insert("abc");
        b.mark_undo_point();
        assert_eq!(b.text(), "abc");
        b.redo(); // Do nothing.
        assert_eq!(b.text(), "abc");
        b.undo();
        assert_eq!(b.text(), "");
        b.redo();
        assert_eq!(b.text(), "abc");

        let mut b = Buffer::new();
        b.insert("abc");
        b.mark_undo_point();
        b.insert("123");
        b.mark_undo_point();
        b.insert("xyz");
        b.mark_undo_point();
        assert_eq!(b.text(), "abc123xyz");
        b.undo();
        assert_eq!(b.text(), "abc123");
        b.undo();
        assert_eq!(b.text(), "abc");
        b.redo();
        assert_eq!(b.text(), "abc123");
        b.redo();
        assert_eq!(b.text(), "abc123xyz");
        b.undo();
        assert_eq!(b.text(), "abc123");
        b.undo();
        assert_eq!(b.text(), "abc");
        b.undo();
        assert_eq!(b.text(), "");
        b.undo();
        assert_eq!(b.text(), "");
    }
}
