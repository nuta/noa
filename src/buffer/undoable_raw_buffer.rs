use std::ops::Deref;

use crate::{
    cursor::{Cursor, Position, Range},
    raw_buffer::RawBuffer,
};

#[derive(Clone, PartialEq, Debug)]
pub struct Change {
    pub range: Range,
    pub byte_range: std::ops::Range<usize>,
    pub new_pos: Position,
    pub insert_text: String,
}

pub struct UndoableRawBuffer {
    raw: RawBuffer,
    changes: Vec<Change>,
}

impl UndoableRawBuffer {
    pub fn new() -> UndoableRawBuffer {
        UndoableRawBuffer {
            raw: RawBuffer::new(),
            changes: Vec::new(),
        }
    }

    pub fn raw_buffer(&self) -> &RawBuffer {
        &self.raw
    }

    pub fn from_text(text: &str) -> UndoableRawBuffer {
        UndoableRawBuffer {
            raw: RawBuffer::from_text(text),
            changes: Vec::new(),
        }
    }

    pub fn from_reader<T: std::io::Read>(reader: T) -> std::io::Result<UndoableRawBuffer> {
        Ok(UndoableRawBuffer {
            raw: RawBuffer::from_reader(reader)?,
            changes: Vec::new(),
        })
    }

    pub fn clear_changes(&mut self) -> Vec<Change> {
        let changes = self.changes.drain(..).collect();
        self.changes = Vec::new();
        changes
    }

    pub fn edit(&mut self, range: Range, new_text: &str) -> &Change {
        let new_pos = Position::position_after_edit(range, new_text);
        self.changes.push(Change {
            range,
            insert_text: new_text.to_owned(),
            new_pos,
            byte_range: self.raw.pos_to_byte_index(range.front())
                ..self.raw.pos_to_byte_index(range.back()),
        });

        self.raw.edit(range, new_text);
        self.changes.last().unwrap()
    }

    pub fn edit_at_cursor(
        &mut self,
        current_cursor: &mut Cursor,
        past_cursors: &mut [Cursor],
        new_text: &str,
    ) {
        let range_removed = current_cursor.selection();
        let prev_back_y = current_cursor.selection().back().y;

        let change = self.edit(range_removed, new_text);

        // Move the current cursor.
        let new_pos = change.new_pos;
        current_cursor.move_to(new_pos.y, new_pos.x);

        // Adjust past cursors.
        let y_diff = (new_pos.y as isize) - (prev_back_y as isize);
        for c in past_cursors {
            let s = c.selection_mut();

            if s.start.y == range_removed.back().y {
                s.start.x = new_pos.x + (s.start.x - range_removed.back().x);
            }
            if s.end.y == range_removed.back().y {
                s.end.x = new_pos.x + (s.end.x - range_removed.back().x);
            }

            s.start.y = ((s.start.y as isize) + y_diff) as usize;
            s.end.y = ((s.end.y as isize) + y_diff) as usize;
        }
    }
}

impl Default for UndoableRawBuffer {
    fn default() -> UndoableRawBuffer {
        UndoableRawBuffer::new()
    }
}

impl PartialEq for UndoableRawBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Deref for UndoableRawBuffer {
    type Target = RawBuffer;

    fn deref(&self) -> &RawBuffer {
        &self.raw
    }
}
