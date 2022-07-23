use std::{
    cmp::{max, min, Ordering},
    fmt::{Debug, Display},
    hash::{Hash, Hasher},
    sync::atomic::{self, AtomicUsize},
};

use crate::{paragraph_iter::Paragraph, raw_buffer::RawBuffer, reflow_iter::ReflowItem};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Prev,
    Next,
}

/// The zero-based position in the buffer.
///
/// Both `x` and `y` are indices in characters (`char`), not graphemes. For
/// example, the Woman Scientist emoji, represented as three characters
/// (U+1F469 U+200D U+1F52C), occupies 3 (not 1) in x-axis:
///
/// ```text
/// U+1F469 U+200D U+1F52C
/// ^       ^      ^
/// x=0     x=1    x=2
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
        let num_newlines_added = new_text.matches('\n').count();
        let num_newlines_deleted = range.back().y - range.front().y;

        let y_diff = num_newlines_added.saturating_sub(num_newlines_deleted);

        let mut x_diff = 0;
        for c in new_text.chars() {
            if c == '\n' {
                x_diff = 0;
            } else {
                x_diff += 1;
            }
        }

        let new_y = pos.y + y_diff;
        let new_x = if new_text.contains('\n') {
            x_diff
        } else {
            pos.x + x_diff
        };

        Position::new(new_y, new_x)
    }

    fn screen_x(&self, buf: &RawBuffer, screen_width: usize, tab_width: usize) -> usize {
        let iter = buf
            .paragraph_iter(*self, screen_width, tab_width)
            .next()
            .expect("invalid position");

        let mut last_screen_x = 0;
        for ReflowItem {
            pos_in_buffer,
            pos_in_screen,
            ..
        } in iter.reflow_iter
        {
            if pos_in_buffer == *self {
                return pos_in_screen.x;
            }

            last_screen_x = pos_in_screen.x;
        }

        assert!(buf.line_len(self.y) == self.x);
        last_screen_x + 1
    }

    #[must_use]
    pub fn move_horizontally(&self, buf: &RawBuffer, direction: Direction) -> Position {
        let new_pos = match direction {
            Direction::Next => {
                let mut iter = buf.bidirectional_grapheme_iter(*self);
                if iter.next().is_some() {
                    Some(iter.next_position())
                } else {
                    None
                }
            }
            Direction::Prev => {
                let mut iter = buf.bidirectional_grapheme_iter(*self);
                if iter.prev().is_some() {
                    Some(iter.next_position())
                } else {
                    None
                }
            }
        };

        let new_pos = new_pos.unwrap_or(*self);
        debug_assert!(new_pos.y < buf.num_lines());
        debug_assert!(new_pos.x <= buf.line_len(new_pos.y));
        new_pos
    }

    #[must_use]
    fn move_vertically(
        &self,
        buf: &RawBuffer,
        direction: Direction,
        screen_width: usize,
        tab_width: usize,
    ) -> Position {
        let screen_x = self.screen_x(buf, screen_width, tab_width);
        let mut paragraph_iter = buf.paragraph_iter(*self, screen_width, tab_width);
        let new_pos = match direction {
            Direction::Next => {
                // Get the reflow_iter for the current paragraph and move it
                // until the next screen row after the `self` position.
                let reflow_iter = paragraph_iter
                    .next()
                    .unwrap()
                    .reflow_iter
                    .skip_while(|item| item.pos_in_buffer != *self)
                    .skip_while(|item| item.pos_in_screen.x >= screen_x);

                // Current paragraph (soft wrapping).
                match find_same_screen_x(reflow_iter, screen_x) {
                    (true, Some(pos)) => Some(pos),
                    (true, None) => unreachable!(),
                    (false, Some(pos)) => Some(Position::new(pos.y, pos.x + 1)),
                    (false, None) => {
                        // Next paragraph.
                        match paragraph_iter.next() {
                            Some(Paragraph { reflow_iter }) => {
                                match find_same_screen_x(reflow_iter, screen_x) {
                                    (true, Some(pos)) => Some(pos),
                                    (true, None) => unreachable!(),
                                    (false, Some(pos)) => Some(Position::new(pos.y, pos.x + 1)),
                                    (false, None) => None,
                                }
                            }
                            None => None,
                        }
                    }
                }
            }
            Direction::Prev => {
                // Get the reflow_iter for the current paragraph and move it
                // until the previous screen row before the `self` position.
                let mut tmp = [None, None];
                let mut row = 0;
                for ReflowItem {
                    pos_in_buffer,
                    pos_in_screen,
                    ..
                } in paragraph_iter.next().unwrap().reflow_iter
                {
                    if pos_in_screen.x == 0 {
                        row += 1;
                        tmp[row % 2] = Some(pos_in_buffer);
                    }

                    if pos_in_buffer == *self {
                        break;
                    }
                }

                let prev_row_buffer_pos = tmp[(row.saturating_sub(1)) % 2];
                match prev_row_buffer_pos {
                    Some(prev_row_buffer_pos) => {
                        dbg!(screen_x, prev_row_buffer_pos);
                        let prev_row_reflow_iter =
                            buf.reflow_iter(prev_row_buffer_pos, screen_width, tab_width);

                        // Current paragraph (soft wrapping).
                        match find_same_screen_x(prev_row_reflow_iter, screen_x) {
                            (true, Some(pos)) => Some(pos),
                            (true, None) => unreachable!(),
                            (false, Some(pos)) => Some(Position::new(pos.y, pos.x + 1)),
                            (false, None) => {
                                // Previous paragraph.
                                match paragraph_iter.next() {
                                    Some(Paragraph { reflow_iter }) => {
                                        match find_same_screen_x(reflow_iter, screen_x) {
                                            (true, Some(pos)) => Some(pos),
                                            (true, None) => unreachable!(),
                                            (false, Some(pos)) => {
                                                Some(Position::new(pos.y, pos.x + 1))
                                            }
                                            (false, None) => None,
                                        }
                                    }
                                    None => None,
                                }
                            }
                        }
                    }
                    None => None,
                }
            }
        };

        let new_pos = new_pos.unwrap_or(*self);
        debug_assert!(new_pos.y < buf.num_lines());
        debug_assert!(new_pos.x <= buf.line_len(new_pos.y));
        new_pos
    }
}

fn find_same_screen_x<'a, I: Iterator<Item = ReflowItem<'a>>>(
    reflow_iter: I,
    screen_x: usize,
) -> (bool, Option<Position>) {
    let mut last_pos_in_buffer = None;
    for ReflowItem {
        pos_in_buffer,
        pos_in_screen,
        ..
    } in reflow_iter
    {
        if pos_in_screen.x > screen_x {
            return (true, Some(last_pos_in_buffer.unwrap_or(pos_in_buffer)));
        } else if pos_in_screen.x == screen_x {
            return (true, Some(pos_in_buffer));
        }

        last_pos_in_buffer = Some(pos_in_buffer);
    }

    (false, last_pos_in_buffer)
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
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    /// The start position.
    pub(crate) start: Position,
    /// The end position. Exclusive.
    pub(crate) end: Position,
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

    pub fn from_single_position(pos: Position) -> Range {
        Range {
            start: pos,
            end: pos,
        }
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
        self.front() <= pos && pos < self.back()
    }

    pub fn contains_or_contacts_with(&self, pos: Position) -> bool {
        self.front() <= pos && pos <= self.back()
    }

    pub fn contains_range(&self, range: Range) -> bool {
        self.front() <= range.front() && range.back() <= self.back()
    }

    pub fn overlaps(&self, pos: Position) -> bool {
        self.overlaps_with(Range::from_positions(pos, pos))
    }

    pub fn overlapped_lines(&mut self) -> std::ops::Range<usize> {
        let front = self.front();
        let back = self.back();

        let end_y = match (front.y == back.y, back.x == 0) {
            (true, _) => front.y,
            (false, true) => back.y,
            (false, false) => back.y + 1,
        };

        front.y..end_y
    }

    pub fn overlaps_with(&self, other: Range) -> bool {
        self == &other
            || !(self.back().y < other.front().y
                || self.front().y > other.back().y
                || (self.back().y == other.front().y && self.back().x <= other.front().x)
                || (self.front().y == other.back().y && self.front().x >= other.back().x))
    }

    pub fn overlaps_or_contacts_with(&self, other: Range) -> bool {
        self.overlaps_with(other)
            || self.contains_or_contacts_with(other.start)
            || self.contains_or_contacts_with(other.end)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CursorId(usize);

/// A text cursor.
#[derive(Clone)]
pub struct Cursor {
    id: CursorId,
    /// The range selected by the cursor. If the cursor is not a selection,
    /// the range is empty.
    selection: Range,
}

impl Debug for Cursor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.selection.is_empty() {
            write!(
                f,
                "Cursor<{}, {}>",
                self.selection.start.y, self.selection.start.x
            )
        } else {
            write!(
                f,
                "Selection<{} - {}>",
                self.selection.start, self.selection.end
            )
        }
    }
}

const MAIN_CURSOR_ID: CursorId = CursorId(0);
static NEXT_CURSOR_ID: AtomicUsize = AtomicUsize::new(1);

impl Cursor {
    fn new_main_cursor(y: usize, x: usize) -> Cursor {
        Cursor {
            id: MAIN_CURSOR_ID,
            selection: Range::new(y, x, y, x),
        }
    }

    pub fn new(y: usize, x: usize) -> Cursor {
        Cursor {
            id: CursorId(NEXT_CURSOR_ID.fetch_add(1, atomic::Ordering::SeqCst)),
            selection: Range::new(y, x, y, x),
        }
    }

    pub fn new_selection(start_y: usize, start_x: usize, end_y: usize, end_x: usize) -> Cursor {
        Cursor {
            id: CursorId(NEXT_CURSOR_ID.fetch_add(1, atomic::Ordering::SeqCst)),
            selection: Range::new(start_y, start_x, end_y, end_x),
        }
    }

    pub fn from_position(pos: Position) -> Cursor {
        Cursor {
            id: CursorId(NEXT_CURSOR_ID.fetch_add(1, atomic::Ordering::SeqCst)),
            selection: Range::from_positions(pos, pos),
        }
    }

    pub fn from_range(selection: Range) -> Cursor {
        Cursor {
            id: CursorId(NEXT_CURSOR_ID.fetch_add(1, atomic::Ordering::SeqCst)),
            selection,
        }
    }

    pub fn id(&self) -> CursorId {
        self.id
    }

    pub fn is_main_cursor(&self) -> bool {
        self.id == MAIN_CURSOR_ID
    }

    pub fn is_selection(&self) -> bool {
        !self.selection.is_empty()
    }

    pub fn selection(&self) -> Range {
        self.selection
    }

    /// Returns the cursor position if it's not a selection.
    pub fn position(&self) -> Option<Position> {
        if self.selection.is_empty() {
            Some(self.selection.start)
        } else {
            None
        }
    }

    pub fn moving_position(&self) -> Position {
        self.selection.end
    }

    pub fn fixed_position(&self) -> Position {
        self.selection.start
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

    pub fn move_to(&mut self, y: usize, x: usize) {
        self.move_to_pos(Position::new(y, x));
    }

    pub fn move_to_pos(&mut self, pos: Position) {
        self.selection.start = pos;
        self.selection.end = pos;
    }

    pub fn select(&mut self, start_y: usize, start_x: usize, end_y: usize, end_x: usize) {
        self.selection = Range::new(start_y, start_x, end_y, end_x);
    }

    pub fn select_range(&mut self, selection: Range) {
        self.selection = selection;
    }

    pub fn move_moving_position_to(&mut self, pos: Position) {
        self.selection.end = pos;
    }

    pub fn move_left(&mut self, buf: &RawBuffer) {
        if self.selection.is_empty() {
            self.selection.start = self.selection.start.move_horizontally(buf, Direction::Prev);
            self.selection.end = self.selection.end.move_horizontally(buf, Direction::Prev);
            assert_eq!(self.selection.start, self.selection.end);
        } else {
            self.move_to_pos(self.selection.front());
        }
    }

    pub fn move_right(&mut self, buf: &RawBuffer) {
        if self.selection.is_empty() {
            self.selection.start = self.selection.start.move_horizontally(buf, Direction::Next);
            self.selection.end = self.selection.end.move_horizontally(buf, Direction::Next);
            assert_eq!(self.selection.start, self.selection.end);
        } else {
            self.move_to_pos(self.selection.back());
        }
    }

    pub fn move_up(&mut self, buf: &RawBuffer, screen_width: usize, tab_width: usize) {
        self.move_to_pos(self.selection.front().move_vertically(
            buf,
            Direction::Prev,
            screen_width,
            tab_width,
        ));
    }

    pub fn move_down(&mut self, buf: &RawBuffer, screen_width: usize, tab_width: usize) {
        self.move_to_pos(self.selection.back().move_vertically(
            buf,
            Direction::Next,
            screen_width,
            tab_width,
        ));
    }

    pub fn expand_left(&mut self, buf: &RawBuffer) {
        let pos = self.selection.front_mut();
        *pos = pos.move_horizontally(buf, Direction::Prev);
    }

    pub fn expand_right(&mut self, buf: &RawBuffer) {
        let pos = self.selection.back_mut();
        *pos = pos.move_horizontally(buf, Direction::Next);
    }

    pub fn select_overlapped_lines(&mut self) {
        let mut front = self.selection.front();
        let mut back = self.selection.back();

        if front.y == back.y || back.x > 0 {
            back.y += 1;
            back.x = 0;
        }
        front.x = 0;

        self.selection.start = front;
        self.selection.end = back;
    }
}

impl PartialEq for Cursor {
    fn eq(&self, other: &Cursor) -> bool {
        self.selection.front() == other.selection.front()
    }
}

impl Hash for Cursor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // It's safe because no multiple cursors can be at the same position
        // (self.selection.front()).
        self.selection.front().hash(state);
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

#[derive(Clone, Debug)]
struct CursorUndoState {
    cursors: Vec<Cursor>,
}

/// A set of cursors, so-called multiple cursors.
#[derive(Clone, Debug)]
pub struct CursorSet {
    cursors: Vec<Cursor>,
    undo_stack: Vec<CursorUndoState>,
    redo_stack: Vec<CursorUndoState>,
}

impl CursorSet {
    pub fn new() -> CursorSet {
        CursorSet {
            cursors: vec![Cursor::new_main_cursor(0, 0)],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Returns the cursor if there is only one cursor and it's a selection.
    pub fn single_selection_cursor(&self) -> Option<&Cursor> {
        if self.cursors.len() == 1 && self.cursors[0].is_selection() {
            Some(&self.cursors[0])
        } else {
            None
        }
    }

    pub fn main_cursor(&self) -> &Cursor {
        self.cursors.iter().find(|c| c.is_main_cursor()).unwrap()
    }

    pub fn get_cursor_by_id(&self, id: CursorId) -> Option<&Cursor> {
        self.cursors.iter().find(|c| c.id == id)
    }

    pub fn as_slice(&self) -> &[Cursor] {
        &self.cursors
    }

    pub fn add_cursor(&mut self, selection: Range) -> CursorId {
        let mut new_cursors = self.cursors.to_vec();
        let cursor = Cursor::new_selection(
            selection.start.y,
            selection.start.x,
            selection.end.y,
            selection.end.x,
        );
        let id = cursor.id;
        new_cursors.push(cursor);
        self.save_undo_state();
        self.update_cursors(&new_cursors);
        id
    }

    pub fn remove_cursor(&mut self, id: CursorId) {
        let mut new_cursors = self.cursors.to_vec();
        new_cursors.retain(|c| c.id != id);
        self.update_cursors(&new_cursors);
    }

    pub fn clear_secondary_cursors(&mut self) {
        self.update_cursors(&[self.main_cursor().clone()]);
    }

    pub fn set_cursors_for_test(&mut self, new_cursors: &[Cursor]) {
        debug_assert!(!new_cursors.is_empty());
        let mut new_cursors = new_cursors.to_vec();
        new_cursors[0].id = MAIN_CURSOR_ID;
        self.update_cursors(&new_cursors);
    }

    pub fn update_cursors(&mut self, new_cursors: &[Cursor]) {
        self.do_set_cursors(new_cursors);
        debug_assert!(self.cursors.iter().any(|c| c.is_main_cursor()));
    }

    fn do_set_cursors(&mut self, new_cursors: &[Cursor]) {
        debug_assert!(!new_cursors.is_empty());

        // Sort and merge cursors.
        let mut new_cursors = new_cursors.to_vec();
        new_cursors.sort();

        // Remove duplicates.
        let mut i = 0;
        while i < new_cursors.len() - 1 {
            let c = &new_cursors[i];
            let next_c = &new_cursors[i + 1];
            if c.selection().overlaps_or_contacts_with(next_c.selection()) {
                let next_is_main_cursor = next_c.is_main_cursor();

                if c.is_selection() || next_c.is_selection() {
                    let selection = Range::from_positions(
                        min(c.front(), next_c.front()),
                        max(c.back(), next_c.back()),
                    );

                    new_cursors[i].selection = selection;
                }

                if next_is_main_cursor {
                    // next_c will be removed. Preserve the main cursor.
                    new_cursors[i].id = MAIN_CURSOR_ID;
                }

                new_cursors.remove(i + 1);
            } else {
                i += 1;
            }
        }

        self.cursors = new_cursors;
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
        self.update_cursors(&new_cursors);
    }

    pub fn deselect_cursors(&mut self) {
        self.foreach(|cursor, _| {
            if cursor.is_selection() {
                cursor.selection = Range::from_single_position(cursor.moving_position());
            }
        });
    }

    pub fn clear_undo_and_redo_stacks(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn save_undo_state(&mut self) {
        if self.cursors.is_empty() {
            return;
        }

        self.undo_stack.push(CursorUndoState {
            cursors: self.cursors.clone(),
        });
    }

    pub fn undo_cursor_movements(&mut self) {
        while let Some(state) = self.undo_stack.pop() {
            if self.cursors == state.cursors {
                continue;
            }

            self.do_set_cursors(&state.cursors);
            debug_assert!(self.cursors.iter().any(|c| c.is_main_cursor()));
            self.redo_stack.push(state);
            break;
        }
    }

    pub fn redo_cursor_movements(&mut self) {
        while let Some(state) = self.redo_stack.pop() {
            if self.cursors == state.cursors {
                continue;
            }

            self.do_set_cursors(&state.cursors);
            debug_assert!(self.cursors.iter().any(|c| c.is_main_cursor()));
            self.undo_stack.push(state);
            break;
        }
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
    use crate::buffer::Buffer;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn position_ordering() {
        assert!(Position::new(0, 0) < Position::new(0, 1));
        assert!(Position::new(0, 123) < Position::new(1, 0));
        assert!(Position::new(1, 0) < Position::new(2, 0));
    }

    #[test]
    fn range_overlaps_with() {
        let a = Range::new(0, 1, 0, 1);
        let b = Range::new(0, 1, 0, 1);
        assert!(a.overlaps_with(b));

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

    #[test]
    fn select_overlapped_lines() {
        let mut cursor = Cursor::new(0, 0);
        cursor.select_overlapped_lines();
        assert_eq!(cursor.selection(), Range::new(0, 0, 1, 0));

        let mut cursor = Cursor::new(0, 2);
        cursor.select_overlapped_lines();
        assert_eq!(cursor.selection(), Range::new(0, 0, 1, 0));
    }

    #[test]
    fn test_cursor_uniqueness() {
        let mut cursors = CursorSet::new();
        cursors.set_cursors_for_test(&[Cursor::new(0, 2)]);
        cursors.add_cursor(Range::new(0, 0, 0, 2));
        assert_eq!(cursors.cursors, vec![Cursor::new_selection(0, 0, 0, 2)]);
    }

    #[test]
    fn screen_x() {
        // abcde
        // xyz
        // 123
        let buf = Buffer::from_text("abcdxyz\n123");
        let screen_width = 5;
        let tab_width = 4;
        assert_eq!(
            Position::new(0, 0).screen_x(&buf, screen_width, tab_width),
            0
        );
        assert_eq!(
            Position::new(0, 4).screen_x(&buf, screen_width, tab_width),
            4
        );
        assert_eq!(
            Position::new(0, 5).screen_x(&buf, screen_width, tab_width),
            0
        );
        assert_eq!(
            Position::new(1, 3).screen_x(&buf, screen_width, tab_width),
            3
        );
    }

    #[test]
    fn move_down() {
        // abcde
        // xyz
        // a
        let buf = Buffer::from_text("abcd\nxyz\na");
        let screen_width = 5;
        let tab_width = 4;
        assert_eq!(
            Position::new(0, 1).move_vertically(&buf, Direction::Next, screen_width, tab_width),
            Position::new(1, 1)
        );
        assert_eq!(
            Position::new(1, 2).move_vertically(&buf, Direction::Next, screen_width, tab_width),
            Position::new(2, 1)
        );
        assert_eq!(
            Position::new(2, 1).move_vertically(&buf, Direction::Next, screen_width, tab_width),
            Position::new(2, 1)
        );
    }

    #[test]
    fn move_down_wrapped() {
        // abcde
        // xyz
        let buf = Buffer::from_text("abcdexyz");
        let screen_width = 5;
        let tab_width = 4;
        assert_eq!(
            Position::new(0, 2).move_vertically(&buf, Direction::Next, screen_width, tab_width),
            Position::new(0, 7)
        );
        assert_eq!(
            Position::new(0, 4).move_vertically(&buf, Direction::Next, screen_width, tab_width),
            Position::new(0, 8)
        );
        assert_eq!(
            Position::new(0, 7).move_vertically(&buf, Direction::Next, screen_width, tab_width),
            Position::new(0, 7)
        );
    }

    #[test]
    fn move_up() {
        // xyz
        // abcde
        let buf = Buffer::from_text("xyz\nabcde");
        let screen_width = 5;
        let tab_width = 4;
        // assert_eq!(
        //     Position::new(1, 1).move_vertically(&buf, Direction::Prev, screen_width, tab_width),
        //     Position::new(0, 1)
        // );
        // assert_eq!(
        //     Position::new(1, 4).move_vertically(&buf, Direction::Prev, screen_width, tab_width),
        //     Position::new(0, 3)
        // );
        // assert_eq!(
        //     Position::new(0, 1).move_vertically(&buf, Direction::Prev, screen_width, tab_width),
        //     Position::new(0, 1)
        // );
    }

    #[test]
    fn move_up_wrapped() {
        // abcde
        // xyz
        let buf = Buffer::from_text("abcdexyz");
        let screen_width = 5;
        let tab_width = 4;
        assert_eq!(
            Position::new(0, 7).move_vertically(&buf, Direction::Prev, screen_width, tab_width),
            Position::new(0, 2)
        );
        assert_eq!(
            Position::new(0, 8).move_vertically(&buf, Direction::Prev, screen_width, tab_width),
            Position::new(0, 3)
        );
        assert_eq!(
            Position::new(0, 1).move_vertically(&buf, Direction::Prev, screen_width, tab_width),
            Position::new(0, 1)
        );
    }
}
