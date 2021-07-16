use std::cmp::Ordering;
use std::cmp::{max, min};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::rope::Rope;

/// The position in the text buffer (0-origin).
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub y: usize,
    /// If it's `std::usize::MAX`, it points to the end of a line.
    pub x: usize,
}

impl Point {
    pub fn new(y: usize, x: usize) -> Point {
        Point { y, x }
    }

    pub fn move_by(&mut self, rope: &Rope, up: usize, down: usize, left: usize, right: usize) {
        let num_lines = rope.num_lines();
        if right > 0 {
            let mut r = right;
            loop {
                let max_x = rope.line_len(self.y);
                if self.x + r <= max_x {
                    self.x += r;
                    break;
                } else if self.y >= num_lines {
                    break;
                } else {
                    r -= max_x - self.x + 1;
                    self.x = 0;
                    self.y += 1;
                }
            }
        }

        if left > 0 {
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
            match a.y.cmp(&b.y) {
                Ordering::Less => Ordering::Less,
                Ordering::Greater => Ordering::Greater,
                Ordering::Equal => a.x.cmp(&b.x),
            }
        }
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Point) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A text range where `front < back` and `[front, end)`.
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
        Range { start, end }
    }

    pub fn front(&self) -> Point {
        min(self.start, self.end)
    }

    pub fn back(&self) -> Point {
        max(self.start, self.end)
    }

    pub fn contains(&self, pos: Point) -> bool {
        self.start <= pos && pos < self.end
    }

    pub fn contains_range(&self, other: &Range) -> bool {
        trace!(
            "CONTAINS: {} {}, {} {}",
            self,
            other,
            self.front() <= other.front(),
            other.back() <= self.back()
        );
        self.front() <= other.front() && other.back() <= self.back()
    }

    pub fn is_empty(&self) -> bool {
        self.front() == self.back()
    }

    pub fn overlaps_with(&self, other: &Range) -> bool {
        !(self.back().y < other.front().y
            || self.front().y > other.back().y
            || (self.back().y == other.front().y && self.back().x <= other.front().x)
            || (self.front().y == other.back().y && self.front().x >= other.back().x))
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end)
    }
}

impl PartialOrd for Range {
    fn partial_cmp(&self, other: &Range) -> Option<Ordering> {
        self.front().partial_cmp(&other.front())
    }
}

impl Ord for Range {
    fn cmp(&self, other: &Range) -> Ordering {
        self.front().cmp(&other.front())
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Cursor {
    Normal { pos: Point },
    Selection(Range),
}

impl Cursor {
    pub fn new(y: usize, x: usize) -> Cursor {
        Cursor::Normal {
            pos: Point::new(y, x),
        }
    }

    pub fn front(&self) -> Point {
        match self {
            Cursor::Normal { pos, .. } => *pos,
            Cursor::Selection(range) => range.front(),
        }
    }

    pub fn back(&self) -> Point {
        match self {
            Cursor::Normal { pos, .. } => *pos,
            Cursor::Selection(range) => range.back(),
        }
    }

    pub fn anchor(&self) -> Point {
        match self {
            Cursor::Normal { pos, .. } => *pos,
            Cursor::Selection(Range { start, .. }) => *start,
        }
    }
}

impl From<Point> for Cursor {
    fn from(pos: Point) -> Cursor {
        Cursor::new(pos.y, pos.x)
    }
}

impl Ord for Cursor {
    fn cmp(&self, other: &Cursor) -> Ordering {
        let a = match self {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection(Range { start, .. }) => start,
        };

        let b = match other {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection(Range { start, .. }) => start,
        };

        a.cmp(b)
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Cursor) -> Option<Ordering> {
        Some(self.cmp(other))
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
            Cursor::new(1, 2),
            Cursor::new(0, 3),
            Cursor::new(2, 1),
            Cursor::new(0, 1),
        ]);
        assert_eq!(
            b.cursors(),
            &[
                Cursor::new(0, 1),
                Cursor::new(0, 3),
                Cursor::new(1, 2),
                Cursor::new(2, 1),
            ]
        );
    }

    #[test]
    fn range_overlaps_with() {
        let a = Range::new(0, 0, 0, 2);
        let b = Range::new(0, 1, 0, 3);
        assert_eq!(a.overlaps_with(&b), true);

        let a = Range::new(0, 0, 0, 1);
        let b = Range::new(0, 0, 0, 1);
        assert_eq!(a.overlaps_with(&b), true);

        let a = Range::new(0, 0, 0, 2);
        let b = Range::new(0, 2, 0, 3);
        assert_eq!(a.overlaps_with(&b), false);

        let a = Range::new(0, 0, 0, 2);
        let b = Range::new(0, 3, 0, 4);
        assert_eq!(a.overlaps_with(&b), false);
    }
}
