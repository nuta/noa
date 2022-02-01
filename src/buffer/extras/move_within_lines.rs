use crate::buffer::Buffer;

impl Buffer {
    pub fn move_to_end_of_line(&mut self) {
        self.cursors.foreach(|c, _past_cursors| {
            let y = c.moving_position().y;
            c.move_to(y, self.buf.line_len(y));
        });
    }

    pub fn move_to_beginning_of_line(&mut self) {
        self.cursors.foreach(|c, _past_cursors| {
            c.move_to(c.moving_position().y, 0);
        });
    }
}
