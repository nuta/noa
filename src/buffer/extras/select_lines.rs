use crate::{
    buffer::Buffer,
    cursor::{Position, Range},
};

impl Buffer {
    pub fn select_whole_line(&mut self, pos: Position) {
        let range = Range::new(pos.y, 0, pos.y + 1, 0);
        self.select_main_cursor(range);
    }

    pub fn select_whole_buffer(&mut self) {
        let range = Range::new(0, 0, self.num_lines(), 0);
        self.select_main_cursor(range);
    }
}
