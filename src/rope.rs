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

pub struct CursorSet {
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
                Cursor::Selection(Range { start, .. }) => {
                    *cursor = Cursor::Normal(*start);
                }
            }
        }

        self.sort_and_dedup();
        // Reverse the deletion positions to avoid doing error-prone offset
        // calculation cause by deleting newlines.
        cursors.reverse();
        cursors
    }

    pub fn move_by_delete(&mut self, rope: &Rope) -> Vec<Cursor> {
        let mut cursors = self.cursors.clone();
        let mut num_newlines_deleted = 0;
        let max_y = rope.num_lines();
        // let mut prev_y = None;
        // let mut prev_len = None;
        for cursor in &mut self.cursors {
            match cursor {
                Cursor::Normal(pos) => {
                    let max_x = rope.line_len(pos.y);
                    let end = if pos.y == max_y && pos.x == max_x {
                        pos.y -= num_newlines_deleted;
                        continue;
                    } else if pos.x == max_x {
                        num_newlines_deleted += 1;
                        pos.y -= num_newlines_deleted;
                        pos.x = rope.line_len(pos.y);
                    } else {
                        // Nothing to do
                        pos.y -= num_newlines_deleted;
                    };
                }
                Cursor::Selection(Range { start, .. }) => {
                    *cursor = Cursor::Normal(*start);
                }
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
pub struct Rope(ropey::Rope);

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

    pub fn remove(&mut self, range: &Range) {
        dbg!(range);
        let start = self.index_in_rope(&range.start);
        let end = self.index_in_rope(&range.end);
        self.0.remove(start..end);
    }

    fn index_in_rope(&self, pos: &Point) -> usize {
        self.0.line_to_char(pos.y) + pos.x
    }
}
