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

    pub fn position_after_edit(range: Range, new_text: &str) -> Position {
        let pos = range.front();
        let string_count = new_text.chars().count();
        let num_newlines_added = new_text.matches('\n').count();
        let num_newlines_deleted = range.back().y - range.front().y;
        let y_diff = num_newlines_added.saturating_sub(num_newlines_deleted);

        let x_diff = new_text
            .rfind('\n')
            .map(|x| string_count - x - 1)
            .unwrap_or(string_count);

        let new_y = pos.y + y_diff;
        let new_x = if new_text.contains('\n') {
            x_diff
        } else {
            pos.x + x_diff
        };

        Position::new(new_y, new_x)
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

    pub fn from_positions(start: Position, end: Position) -> Range {
        Range { start, end }
    }

    pub fn front(&self) -> Position {
        min(self.start, self.end)
    }

    pub fn back(&self) -> Position {
        max(self.start, self.end)
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn overlaps_with(&self, other: Range) -> bool {
        !(self.back().y < other.front().y
            || self.front().y > other.back().y
            || (self.back().y == other.front().y && self.back().x <= other.front().x)
            || (self.front().y == other.back().y && self.front().x >= other.back().x))
    }
}

impl Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end)
    }
}

/// A text cursor.
#[derive(Clone)]
pub struct Cursor {
    /// The range selected by the cursor. If the cursor is not a selection,
    /// the range is empty.
    selection: Range,
}

impl Cursor {
    pub fn new() -> Cursor {
        Cursor {
            selection: Range::new(0, 0, 0, 0),
        }
    }

    fn from_position(pos: Position) -> Cursor {
        Cursor {
            selection: Range::from_positions(pos, pos),
        }
    }

    pub fn selection(&self) -> Range {
        self.selection
    }
}

impl PartialEq for Cursor {
    fn eq(&self, other: &Cursor) -> bool {
        self.selection.front() == other.selection.front()
    }
}

impl Eq for Cursor {}

impl Ord for Cursor {
    fn cmp(&self, other: &Cursor) -> Ordering {
        self.selection.front().cmp(&other.selection.front())
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Cursor) -> Option<Ordering> {
        Some(self.cmp(other))
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

    pub fn use_and_move_cursors<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Cursor) -> Position,
    {
        let mut new_cursors = Vec::new();
        for cursor in self.cursors.iter_mut().rev() {
            new_cursors.push(Cursor::from_position(f(cursor)));
        }

        debug_assert!(!new_cursors.is_empty());

        new_cursors.sort();
        let duplicated = new_cursors.iter().enumerate().map(|(i, c)| {
            (&new_cursors[..i])
                .iter()
                .any(|other| other.selection().overlaps_with(other.selection()))
        });

        self.cursors.clear();
        for (cursor, skip) in new_cursors.iter().zip(duplicated) {
            if !skip {
                self.cursors.push(cursor.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_overlaps_with() {
        let a = Range::new(0, 0, 0, 2);
        let b = Range::new(0, 1, 0, 3);
        assert_eq!(a.overlaps_with(b), true);

        let a = Range::new(0, 0, 0, 1);
        let b = Range::new(0, 0, 0, 1);
        assert_eq!(a.overlaps_with(b), true);

        let a = Range::new(0, 0, 0, 2);
        let b = Range::new(0, 2, 0, 3);
        assert_eq!(a.overlaps_with(b), false);

        let a = Range::new(0, 0, 0, 2);
        let b = Range::new(0, 3, 0, 4);
        assert_eq!(a.overlaps_with(b), false);
    }
}
