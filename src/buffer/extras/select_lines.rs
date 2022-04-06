use crate::{
    buffer::Buffer,
    cursor::{Position, Range},
};

impl Buffer {
    pub fn select_whole_line(&mut self, pos: Position) {
        let range = Range::new(pos.y, 0, pos.y + 1, 0);
        self.select_main_cursor_range(range);
    }

    pub fn select_whole_buffer(&mut self) {
        let end_y = self.num_lines() - 1;
        let range = Range::new(0, 0, end_y, self.line_len(end_y));
        self.select_main_cursor_range(range);
    }
}

#[cfg(test)]
mod tests {
    use crate::cursor::Cursor;

    use super::*;

    #[test]
    fn test_select_whole_buffer() {
        let mut buf = Buffer::from_text("");
        buf.select_whole_buffer();
        assert_eq!(buf.cursors(), &[Cursor::new(0, 0)]);

        let mut buf = Buffer::from_text("hello world");
        buf.select_whole_buffer();
        assert_eq!(buf.cursors(), &[Cursor::new_selection(0, 0, 11, 0)]);

        let mut buf = Buffer::from_text("hello\n");
        buf.select_whole_buffer();
        assert_eq!(buf.cursors(), &[Cursor::new_selection(0, 0, 1, 0)]);
    }
}
