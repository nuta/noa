use std::fmt;
use std::cmp::min;
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

    pub fn overlaps_with(&self, other: &Range) -> bool {
        self.end.y < other.start.y
        || self.start.y > other.end.y
        || (self.end.y == other.start.y && self.end.x < other.start.x)
        || (self.start.y == other.end.y && self.start.x > other.end.x)
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

    pub fn move_by_offsets(
        &mut self,
        rope: &Rope,
        up: usize,
        down: usize,
        left: usize,
        right: usize
    ) {
        let num_lines = rope.len_lines();
        for cursor in &mut self.cursors {
            let mut new_pos = match cursor {
                Cursor::Normal(pos) => {
                    pos.y = pos.y.saturating_add(down);
                    pos.y = pos.y.saturating_sub(up);
                    pos.x = pos.x.saturating_add(right);
                    pos.x = pos.x.saturating_sub(left);
                    *pos
                }
                Cursor::Selection(Range { start, end }) => {
                    *start
                }
            };

            dbg!(new_pos, rope.len_chars(), num_lines);
            new_pos.y = min(new_pos.y, num_lines);
            new_pos.x = min(new_pos.y, rope.line(new_pos.y).len_chars());
        }

        for i in 0..rope.len_lines() {
            dbg!(i, rope.line(i).len_chars());
        }

        self.dedup();
    }

    pub fn move_by_insertion(&mut self, string: &str) -> Vec<Cursor> {
        let y_diff = string.matches('\n').count();
        let x_diff = string.rfind('\n')
            .map(|x| string.len() - x - 1)
            .unwrap_or(string.len());

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

    fn dedup(&mut self) {
        let duplicated =
            self.cursors
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    match c {
                        Cursor::Normal(pos) => {
                            (&self.cursors[(i+1)..])
                                .iter()
                                .any(|other| {
                                    match other {
                                        Cursor::Normal(ref other) => {
                                            *pos == *other
                                        }
                                        _ => unreachable!()
                                    }
                                })
                        }
                        Cursor::Selection(range) => {
                            (&self.cursors[(i+1)..])
                                .iter()
                                .any(|other| {
                                    match other {
                                        Cursor::Selection(ref other) => {
                                            range.overlaps_with(other)
                                        }
                                        _ => unreachable!()
                                    }
                                })
                        }
                    }
                });

        let mut new_cursors = Vec::new();
        for (cursor, skip) in self.cursors.iter().zip(duplicated) {
            if !skip {
                new_cursors.push(cursor.clone());
            }
        }

        self.cursors = new_cursors;
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

    pub fn cursors(&self) -> &[Cursor] {
        self.cursors.cursors()
    }

    pub fn move_cursors(
        &mut self,
        up: usize,
        down: usize,
        left: usize,
        right: usize
    ) {
        self.cursors.move_by_offsets(&self.buf, up, down, left, right);
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

    pub fn backspace(&mut self) {
        /*
        for cursor in self.cursors.move_by_deletion(-1) {
            match cursor {
                Cursor::Normal(pos) => {
                    self.delete_range(&Range::from_points(pos, pos));
                }
                Cursor::Selection(range) => {
                    self.delete_range(&range);
                }
            };
        }
        */
    }

    pub fn mark_undo_point(&mut self) {
        self.undo_stack.push(self.buf.clone());
    }

    pub fn undo(&mut self) {
        if self.undo_stack.len() == 1 && self.buf.len_chars() == 0 {
            return;
        }

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
    fn insertion() {
        let mut b = Buffer::new();
        b.insert("Hello");
        b.insert(" World!");
        assert_eq!(b.text(), "Hello World!");
    }

    #[test]
    fn single_cursor() {
        let mut b = Buffer::new();
        b.move_cursors(1, 0, 0, 0); // Do nothing
        b.insert("A\nDEF\n12345");
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(2, 5))]);
        b.move_cursors(0, 0, 1, 0); // Move right
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(2, 4))]);
        b.move_cursors(1, 0, 0, 0); // Move up
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(1, 3))]);
        b.move_cursors(0, 3, 0, 0); // Move down
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(3, 0))]);
    }

    #[test]
    fn undo() {
        let mut b = Buffer::new();
        b.redo();
        b.undo();
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
        b.undo();
        assert_eq!(b.text(), "");
        b.undo();
        assert_eq!(b.text(), "");
        b.redo();
        assert_eq!(b.text(), "abc");
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
