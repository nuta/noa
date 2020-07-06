use std::fmt;
use std::cmp::min;
use std::collections::HashSet;

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

    fn move_by_offsets(
        &mut self,
        rope: &Rope,
        up: usize,
        down: usize,
        left: usize,
        right: usize
    ) {
        let num_lines = rope.num_lines();
        let mut r = right;
        loop {
            let max_x = rope.line_len(self.y);
            if self.x + r <= max_x {
                self.x += r;
                break;
            } if self.y >= num_lines {
                break;
            } else {
                r -= max_x - self.x;
                self.x = 0;
                self.y += 1;
            }
        }

        let mut l = left;
        loop {
            if l <= self.x {
                self.x -= l;
                break;
            } else if self.y == 0 {
                break;
            } else {
                l -= self.x;
                if l > 0 {
                    l -= 1;
                    self.y -= 1;
                    self.x = rope.line_len(self.y);
                }
            }
        }

        self.y = self.y.saturating_add(down);
        self.y = self.y.saturating_sub(up);

        self.y = min(self.y, num_lines);
        self.x = min(self.x, rope.line_len(self.y));
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
pub enum Cursor {
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
        for cursor in &mut self.cursors {
            // Cancel the selection.
            match cursor {
                Cursor::Normal(_) => {}
                Cursor::Selection(Range { start, end }) => {
                    *cursor = Cursor::Normal(*start)
                }
            };

            // Move the cursor.
            let mut new_pos = match cursor {
                Cursor::Normal(pos) => {
                    pos.move_by_offsets(rope, up, down, left, right);
                }
                Cursor::Selection(_) => unreachable!()
            };
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

#[derive(Clone, Debug, PartialEq)]
struct Rope(ropey::Rope);

impl Rope {
    pub fn new() -> Rope {
        Rope(ropey::Rope::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.len_chars() == 0
    }

    /// Returns the number of characters in the buffer.
    pub fn len(&self) -> usize {
        self.0.len_chars()
    }

    /// Returns the number of characters in the buffer.
    pub fn num_lines(&self) -> usize {
        self.0.len_lines()
    }

    /// Returns a line except new line characters.
    pub fn line(&self, line: usize) -> ropey::RopeSlice<'_> {
        let slice = self.0.line(line);

        // The slice contains newline characters. Trim them.
        let mut len = slice.len_chars();
        while len > 0 {
            if slice.char(len - 1) != '\n' {
                break;
            }

            len -= 1;
        }

        slice.slice(..len)
    }

    /// Returns the number of characters in a line except new line characters.
    pub fn line_len(&self, line: usize) -> usize {
        if line == self.num_lines() {
            0
        } else {
            self.line(line).len_chars()
        }
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }

    pub fn insert(&mut self, pos: &Point, string: &str) {
        self.0.insert(self.index_in_rope(pos), string);
    }

    fn remove(&mut self, range: &Range) {
        let start = self.index_in_rope(&range.start);
        let end = self.index_in_rope(&range.end);
        self.0.remove(start..end);
    }

    fn index_in_rope(&self, pos: &Point) -> usize {
        self.0.line_to_char(pos.y) + pos.x
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
                    self.buf.insert(&pos, string);
                }
                Cursor::Selection(range) => {
                    self.buf.remove(&range);
                    self.buf.insert(&range.start, string);
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
        if self.undo_stack.len() == 1 && self.buf.is_empty() {
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
        b.move_cursors(0, 0, 1, 0); // Move left
        assert_eq!(b.cursors(), &[Cursor::Normal(Point::new(2, 5))]);
        b.move_cursors(0, 0, 0, 1); // Move right
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
