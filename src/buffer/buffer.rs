use crate::{
    cursor::{CursorSet, Position},
    raw_buffer::RawBuffer,
};

pub struct Buffer {
    buf: RawBuffer,
    cursors: CursorSet,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            buf: RawBuffer::new(),
            cursors: CursorSet::new(),
        }
    }

    pub fn insert_str(&mut self, s: &str) {
        self.cursors.use_and_move_cursors(|c| {
            self.buf.edit(c.selection(), s);
            Position::position_after_edit(c.selection(), s)
        });
    }
}
