use std::{
    cmp::{max, min, Ordering},
    fmt::{Debug, Display},
};

use crate::raw_buffer::RawBuffer;

/// The zero-based position in the buffer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// The line number. 0-origin.
    pub y: usize,
    /// The column number. 0-origin.
    pub x: usize,
}

impl Position {
    pub fn new(y: usize, x: usize) -> Position {
        Position { y, x }
    }

    /// Computes the cursor position after the given edit, specifically,
    /// after replacing `range` with `new_text`.
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

    /// Return the new position after moving up/down/left/right.
    pub fn move_by(&mut self, buf: &RawBuffer, up: usize, down: usize, left: usize, right: usize) {
        let num_lines = buf.num_lines();
        if right > 0 {
            let mut iter = buf.grapheme_iter(*self);
            loop {
                match iter.next() {
                    Some(s) if s.as_str() == "\n" => {
                        continue;
                    }
                    Some(_) => {
                        *self = iter.position();
                        break;
                    }
                    None => {
                        // `self` is already at EOF. No need to update.
                        break;
                    }
                }
            }
        }

        if left > 0 {
            let mut iter = buf.grapheme_iter(*self);
            loop {
                match iter.prev() {
                    Some(s) if s.as_str() == "\n" => {
                        continue;
                    }
                    Some(_) => {
                        *self = iter.position();
                        break;
                    }
                    None => {
                        // `self` is already at EOF. No need to update.
                        break;
                    }
                }
            }
        }

        self.y = self.y.saturating_add(down);
        self.y = self.y.saturating_sub(up);

        self.y = min(self.y, num_lines);
        self.x = min(self.x, buf.line_len(self.y));
    }
}

impl Debug for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
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

    pub fn front_mut(&mut self) -> &mut Position {
        min(&mut self.start, &mut self.end)
    }

    pub fn back(&self) -> Position {
        max(self.start, self.end)
    }

    pub fn back_mut(&mut self) -> &mut Position {
        max(&mut self.start, &mut self.end)
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn contains(&self, pos: Position) -> bool {
        self.start <= pos && pos < self.end
    }

    pub fn overlaps_with(&self, other: Range) -> bool {
        !(self.back().y < other.front().y
            || self.front().y > other.back().y
            || (self.back().y == other.front().y && self.back().x <= other.front().x)
            || (self.front().y == other.back().y && self.front().x >= other.back().x))
    }
}

impl Debug for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
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

impl Debug for Cursor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.selection.is_empty() {
            write!(f, "Cursor[{}]", self.selection.start)
        } else {
            write!(
                f,
                "Selection[{}, {}]",
                self.selection.start, self.selection.end
            )
        }
    }
}

impl Cursor {
    pub fn new(y: usize, x: usize) -> Cursor {
        Cursor {
            selection: Range::new(y, x, y, x),
        }
    }

    pub fn new_selection(start_y: usize, start_x: usize, end_y: usize, end_x: usize) -> Cursor {
        Cursor {
            selection: Range::new(start_y, start_x, end_y, end_x),
        }
    }

    pub fn selection(&self) -> Range {
        self.selection
    }

    pub(crate) fn selection_mut(&mut self) -> &mut Range {
        &mut self.selection
    }

    pub fn front(&self) -> Position {
        self.selection.front()
    }

    pub fn back(&self) -> Position {
        self.selection.back()
    }

    pub fn expand_left(&mut self, buf: &RawBuffer) {
        self.selection.front_mut().move_by(buf, 0, 0, 1, 0)
    }

    pub fn expand_right(&mut self, buf: &RawBuffer) {
        self.selection.back_mut().move_by(buf, 0, 0, 0, 1)
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
#[derive(Clone, Debug)]
pub struct CursorSet {
    cursors: Vec<Cursor>,
}

impl CursorSet {
    pub fn new() -> CursorSet {
        CursorSet {
            cursors: vec![Cursor::new(0, 0)],
        }
    }

    pub fn as_slice(&self) -> &[Cursor] {
        &self.cursors
    }

    pub fn set_cursors(&mut self, new_cursors: &[Cursor]) {
        debug_assert!(!new_cursors.is_empty());

        // Sort and merge cursors.
        let mut new_cursors = new_cursors.to_vec();
        new_cursors.sort();
        let duplicated = new_cursors.iter().enumerate().map(|(i, c)| {
            (&new_cursors[..i])
                .iter()
                .any(|other| c.selection().overlaps_with(other.selection()))
        });

        // Update cursors.
        self.cursors.clear();
        for (cursor, skip) in new_cursors.iter().zip(duplicated) {
            if !skip {
                self.cursors.push(cursor.clone());
            }
        }

        debug_assert!(!self.cursors.is_empty());
    }

    pub fn foreach<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Cursor, &mut [Cursor]),
    {
        let mut new_cursors = Vec::new();
        for mut cursor in self.cursors.drain(..).rev() {
            f(&mut cursor, &mut new_cursors);
            new_cursors.push(cursor);
        }
        self.set_cursors(&new_cursors);
    }
}

impl Default for CursorSet {
    fn default() -> CursorSet {
        CursorSet::new()
    }
}

impl<'a> IntoIterator for &'a CursorSet {
    type Item = &'a Cursor;
    type IntoIter = std::slice::Iter<'a, Cursor>;

    fn into_iter(self) -> Self::IntoIter {
        self.cursors.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_overlaps_with() {
        let a = Range::new(0, 0, 0, 2);
        let b = Range::new(0, 1, 0, 3);
        assert!(a.overlaps_with(b));

        let a = Range::new(0, 0, 0, 1);
        let b = Range::new(0, 0, 0, 1);
        assert!(a.overlaps_with(b));

        let a = Range::new(0, 0, 0, 2);
        let b = Range::new(0, 2, 0, 3);
        assert!(!a.overlaps_with(b));

        let a = Range::new(0, 0, 0, 2);
        let b = Range::new(0, 3, 0, 4);
        assert!(!a.overlaps_with(b));
    }
}
