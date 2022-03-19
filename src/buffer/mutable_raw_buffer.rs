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

/// An internal mutable buffer implementation supporting primitive operations
/// required by the editor.
pub struct MutableRawBuffer {
    raw: RawBuffer,
    changes: Vec<Change>,
}

impl MutableRawBuffer {
    pub fn new() -> MutableRawBuffer {
        MutableRawBuffer {
            raw: RawBuffer::new(),
            changes: Vec::new(),
        }
    }

    pub fn from_raw_buffer(raw_buffer: RawBuffer) -> MutableRawBuffer {
        MutableRawBuffer {
            raw: raw_buffer,
            changes: Vec::new(),
        }
    }

    pub fn from_text(text: &str) -> MutableRawBuffer {
        MutableRawBuffer {
            raw: RawBuffer::from_text(text),
            changes: Vec::new(),
        }
    }

    pub fn from_reader<T: std::io::Read>(reader: T) -> std::io::Result<MutableRawBuffer> {
        Ok(MutableRawBuffer {
            raw: RawBuffer::from_reader(reader)?,
            changes: Vec::new(),
        })
    }

    pub fn raw_buffer(&self) -> &RawBuffer {
        &self.raw
    }

    pub fn clear_changes(&mut self) -> Vec<Change> {
        let changes = self.changes.drain(..).collect();
        self.changes = Vec::new();
        changes
    }

    /// Replaces the text at the `range` with `new_text`.
    ///
    /// This is the only method that modifies the buffer.
    ///
    /// # Complexity
    ///
    /// According to the ropey's documentation:
    //
    /// Runs in O(M + log N) time, where N is the length of the Rope and M
    /// is the length of the range being removed/inserted.
    fn edit_without_recording(&mut self, range: Range, new_text: &str) {
        let start = self.pos_to_char_index(range.front());
        let end = self.pos_to_char_index(range.back());

        let mut rope = self.raw.rope().clone();
        if !(start..end).is_empty() {
            rope.remove(start..end);
        }

        if !new_text.is_empty() {
            rope.insert(start, new_text);
        }

        self.raw = RawBuffer::from(rope);
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

        self.edit_without_recording(range, new_text);
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

impl Default for MutableRawBuffer {
    fn default() -> MutableRawBuffer {
        MutableRawBuffer::new()
    }
}

impl PartialEq for MutableRawBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Deref for MutableRawBuffer {
    type Target = RawBuffer;

    fn deref(&self) -> &RawBuffer {
        &self.raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insertion() {
        let mut buffer = MutableRawBuffer::new();
        buffer.edit(Range::new(0, 0, 0, 0), "ABG");
        assert_eq!(buffer.text(), "ABG");

        buffer.edit(Range::new(0, 2, 0, 2), "CDEF");
        assert_eq!(buffer.text(), "ABCDEFG");
    }

    #[test]
    fn test_deletion() {
        let mut buffer = MutableRawBuffer::from_text("ABCDEFG");
        buffer.edit(Range::new(0, 1, 0, 1), "");
        assert_eq!(buffer.text(), "ABCDEFG");

        buffer.edit(Range::new(0, 1, 0, 3), "");
        assert_eq!(buffer.text(), "ADEFG");
    }
}
