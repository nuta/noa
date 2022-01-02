use crate::{cursor::CursorSet, raw_buffer::RawBuffer};

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

    pub fn insert(&mut self, c: char) {
        //
    }
}
