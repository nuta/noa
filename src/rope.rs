use std::fmt;
use std::cmp::{min, max};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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

    pub fn move_by(
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

impl Ord for Point {
    fn cmp(&self, other: &Point) -> Ordering {
        let a = self;
        let b = other;
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
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Point) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
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

    pub fn front(&self) -> &Point {
        min(&self.start, &self.end)
    }

    pub fn end(&self) -> &Point {
        max(&self.start, &self.end)
    }

    pub fn overlaps_with(&self, other: &Range) -> bool {
        !(self.end.y < other.start.y
            || self.start.y > other.end.y
            || (self.end.y == other.start.y && self.end.x < other.start.x)
            || (self.start.y == other.end.y && self.start.x > other.end.x))
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Cursor {
    Normal(Point),
    Selection(Range),
}

impl Ord for Cursor {
    fn cmp(&self, other: &Cursor) -> Ordering {
        let a = match self {
            Cursor::Normal(pos) => {
                pos
            }
            Cursor::Selection(Range { start, .. }) => {
                start
            }
        };

        let b = match other {
            Cursor::Normal(pos) => {
                pos
            }
            Cursor::Selection(Range { start, .. }) => {
                start
            }
        };

        a.cmp(b)
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Cursor) -> Option<Ordering> {
        Some(self.cmp(other))
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
        let start = self.index_in_rope(&range.start);
        let end = self.index_in_rope(&range.end);
        self.0.remove(min(start, end)..max(start, end));
    }

    /// Returns a line except new line characters.
    fn line(&self, line: usize) -> ropey::RopeSlice<'_> {
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

    fn index_in_rope(&self, pos: &Point) -> usize {
        self.0.line_to_char(pos.y) + pos.x
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::buffer::Buffer;

    #[test]
    fn sorting_cursors() {
        // a|bc|
        // de|f
        // x|yz
        let mut b = Buffer::new();
        b.insert("abc\ndef\nxyz");

        // Make sure cursors gets sorted.
        b.set_cursors(vec![
            Cursor::Normal(Point::new(1, 2)),
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(2, 1)),
            Cursor::Normal(Point::new(0, 1)),
        ]);
        assert_eq!(b.cursors(), &[
            Cursor::Normal(Point::new(0, 1)),
            Cursor::Normal(Point::new(0, 3)),
            Cursor::Normal(Point::new(1, 2)),
            Cursor::Normal(Point::new(2, 1)),
        ]);
    }
}
