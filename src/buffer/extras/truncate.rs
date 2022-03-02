use crate::buffer::Buffer;

impl Buffer {
    pub fn truncate(&mut self) {
        self.cursors.foreach(|c, past_cursors| {
            if c.selection().is_empty() {
                // Select until the end of line.
                let pos = c.moving_position();
                let eol = self.buf.line_len(pos.y);
                if pos.x == eol {
                    // The cursor is already at the end of line, remove the
                    // following newline instead.
                    c.select(pos.y, pos.x, pos.y + 1, 0);
                } else {
                    c.select(pos.y, pos.x, pos.y, eol);
                }
            }

            self.buf.edit_at_cursor(c, past_cursors, "");
        });
    }
}
