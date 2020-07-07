use std::fmt;
use std::cmp::min;
use std::cmp::Ordering;
use std::collections::HashMap;

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
            } else if self.y >= num_lines {
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

    pub fn set_cursors(&mut self, cursors: Vec<Cursor>) {
        self.cursors = cursors;
        self.sort_and_dedup();
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

        self.sort_and_dedup();
    }

    pub fn move_by_insertion(&mut self, string: &str) -> Vec<Cursor> {
        let y_diff = string.matches('\n').count();
        let x_diff = string.rfind('\n')
            .map(|x| string.len() - x - 1)
            .unwrap_or_else(|| string.len());

        let mut cursors = Vec::new();
        let mut acc_y_diff = 0;
        let mut acc_x_diff = 0;
        let mut prev_pos: Option<Point> = None;
        for cursor in &mut self.cursors {
            let current = match cursor {
                Cursor::Normal(pos) => {
                    pos
                }
                Cursor::Selection(Range { start, .. }) => {
                    start
                }
            };

            // Handle multiple cursors.
            let mut insert_at = current.clone();
            insert_at.y += acc_y_diff;
            acc_y_diff += y_diff;
            match prev_pos {
                Some(prev) if prev.y == current.y => {
                    insert_at.x += acc_x_diff;
                    acc_x_diff += x_diff;
                }
                _ => {
                    acc_x_diff = 0;
                }
            }

            let mut x = if string.contains('\n') {
                x_diff
            } else {
                insert_at.x + x_diff
            };

            prev_pos = Some(*current);
            let new_pos = Point::new(insert_at.y + y_diff, x);
            *cursor = Cursor::Normal(new_pos);
            cursors.push(Cursor::Normal(insert_at));
        }

        cursors
    }

    pub fn move_by_backspace(&mut self, rope: &Rope) -> Vec<Cursor> {
        let mut cursors = self.cursors.clone();
        let mut num_newlines_deleted = 0;
        for cursor in &mut self.cursors {
            match cursor {
                Cursor::Normal(pos) => {
                    if pos.y == 0 && pos.x == 0 {
                        // Do nothing
                    } else if pos.x == 0 {
                        // Remove a newline.
                        pos.x = rope.line_len(pos.y - 1);
                        num_newlines_deleted += 1;
                        pos.y -= num_newlines_deleted;
                    } else {
                        pos.x -= 1;
                    }
                }
                Cursor::Selection(_) => { /* Do nothing. */ }
            }
        }

        self.sort_and_dedup();
        // Reverse the deletion positions to avoid doing error-prone offset
        // calculation cause by deleting newlines.
        cursors.reverse();
        cursors
    }

    /// Sorts the cursors and removes overlapped ones. Don't forget to call this
    /// method when you made a change.
    fn sort_and_dedup(&mut self) {
        self.cursors.sort_by(|a, b| {
            let a = match a {
                Cursor::Normal(pos) => {
                    pos
                }
                Cursor::Selection(Range { start, .. }) => {
                    start
                }
            };

            let b = match b {
                Cursor::Normal(pos) => {
                    pos
                }
                Cursor::Selection(Range { start, .. }) => {
                    start
                }
            };

            if a == b {
                Ordering::Equal
            } else {
                if a.y < b.y {
                    Ordering::Less
                } else if a.y > b.y {
                    Ordering::Greater
                } else {
                    if a.x < b.x {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                }
            }
        });

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
        dbg!(range);
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

    pub fn set_cursors(&mut self, cursors: Vec<Cursor>) {
        self.cursors.set_cursors(cursors);
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

    pub fn insert_char(&mut self, ch: char) {
        self.insert(&ch.to_string())
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
        for c in self.cursors.move_by_backspace(&self.buf) {
            match c {
                Cursor::Normal(pos) => {
                    let start = if pos.y == 0 && pos.x == 0 {
                        continue;
                    } else if pos.x == 0 {
                        Point::new(pos.y - 1, self.buf.line_len(pos.y - 1))
                    } else {
                        Point::new(pos.y, pos.x - 1)
                    };

                    self.buf.remove(&Range::from_points(start, pos));
                }
                Cursor::Selection(range) => {
                    self.buf.remove(&range);
                }
            };
        }
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
            self.buf = buf;
        }
    }

    pub fn redo(&mut self) {
        if let Some(buf) = self.redo_stack.pop() {
            self.undo_stack.push(self.buf.clone());
            self.buf = buf;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn cursor_set() {
        // .a|b.c|
        // .d.e|f.
        // .x|y.z.
        let mut b = Buffer::new();
        let mut cursors = CursorSet::new();
        b.insert("abc\ndef\nxyz");

        // Make sure cursors gets sorted.
        cursors.set_cursors(vec![
            Cursor::Normal(Point::new(1, 2)),
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(2, 1)),
            Cursor::Normal(Point::new(0, 1)),
        ]);
        assert_eq!(cursors.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(1, 2)),
            Cursor::Normal(Point::new(2, 1)),
        ]);
    }

    #[test]
    fn insertion_and_deletion() {
        let mut b = Buffer::new();
        b.insert("Hello");
        b.insert(" World?");
        assert_eq!(b.text(), "Hello World?");
        b.backspace();
        assert_eq!(b.text(), "Hello World");
        b.insert_char('!');
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
    fn multiple_cursors() {
        let mut b = Buffer::new();
        // abc|
        // d|e
        // |xyz
        b.insert("abc\nde\nxyz");
        b.set_cursors(vec![
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(2, 0)),
        ]);

        // abc123|
        // d123|e
        // 123|xyz
        b.insert("123");
        assert_eq!(b.text(), "abc123\nd123e\n123xyz");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 6)),
            Cursor::Normal(Point::new(1, 4)),
            Cursor::Normal(Point::new(2, 3)),
        ]);

        // abc123[
        // ]|
        // d123[
        // ]|e
        // 123[
        // ]|xyz
        b.insert("[\n]");
        assert_eq!(b.text(), "abc123[\n]\nd123[\n]e\n123[\n]xyz");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(1, 1)),
            Cursor::Normal(Point::new(3, 1)),
            Cursor::Normal(Point::new(5, 1)),
        ]);

        // abc123[
        // |
        // d123[
        // |e
        // 123[
        // |xyz
        b.backspace();
        assert_eq!(b.text(), "abc123[\n\nd123[\ne\n123[\nxyz");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(1, 0)),
            Cursor::Normal(Point::new(3, 0)),
            Cursor::Normal(Point::new(5, 0)),
        ]);

        // abc123[|
        // d123[|e
        // 123[|xyz
        b.backspace();
        assert_eq!(b.text(), "abc123[\nd123[e\n123[xyz");
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 7)),
            Cursor::Normal(Point::new(1, 5)),
            Cursor::Normal(Point::new(2, 4)),
        ]);
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
