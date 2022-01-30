use crate::{
    buffer::Buffer,
    cursor::{Cursor, Position, Range},
};

impl Buffer {
    pub fn select_whole_line(&mut self, pos: Position) {
        let range = Range::new(pos.y, 0, pos.y + 1, 0);
        self.set_main_cursor(Cursor::from_range(range));
    }

    pub fn select_whole_buffer(&mut self) {
        let range = Range::new(0, 0, self.num_lines(), 0);
        self.set_main_cursor(Cursor::from_range(range));
    }
}
