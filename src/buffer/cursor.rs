use std::{
    cmp::{max, min, Ordering},
    fmt::Display,
};

/// The zero-based position in the buffer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// The line number. 0-origin.
    pub y: usize,
    /// The column number. 0-origin.
    ///
    /// If it's `Position::END_OF_LINE`, it means the cursor is at the end of the line.
    pub x: usize,
}

impl Position {
    const END_OF_LINE: usize = std::usize::MAX;

    pub fn new(y: usize, x: usize) -> Position {
        Position { y, x }
    }

    pub fn end_of_line(y: usize) -> Position {
        Position {
            y,
            x: Position::END_OF_LINE,
        }
    }
}

impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.y, self.x)
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Position) -> Ordering {
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

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Position) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// An exclusive range in the buffer.
///
/// Note that `start` don't have to be less (in respect to its `Ord` implementation)
/// than `end`.
#[derive(Clone, Copy, PartialEq)]
pub struct Range {
    /// The start position.
    pub start: Position,
    /// The end position. Exclusive.
    pub end: Position,
}

impl Range {
    pub fn new(start_y: usize, start_x: usize, end_y: usize, end_x: usize) -> Range {
        Range {
            start: Position {
                y: start_y,
                x: start_x,
            },
            end: Position { y: end_y, x: end_x },
        }
    }

    pub fn front(&self) -> Position {
        min(self.start, self.end)
    }

    pub fn back(&self) -> Position {
        max(self.start, self.end)
    }
}

impl Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end)
    }
}

/// A text cursor.
pub struct Cursor {
    selection: Range,
}

impl Cursor {
    pub fn new() -> Cursor {
        Cursor {
            selection: Range::new(0, 0, 0, 0),
        }
    }
}

/// A set of cursors, so-called multiple cursors.
pub struct CursorSet {
    cursors: Vec<Cursor>,
}

impl CursorSet {
    pub fn new() -> CursorSet {
        CursorSet {
            cursors: vec![Cursor::new()],
        }
    }
}
