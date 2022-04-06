use crate::buffer::Buffer;

impl Buffer {
    pub fn duplicate_lines_up(&mut self) {
        self.cursors.foreach(|c, past_cursors| {
            let s = c.selection();

            c.select_overlapped_lines();
            let mut text = self.buf.substr(c.selection());
            if !text.ends_with('\n') {
                text.push('\n');
            }

            c.move_to(s.front().y, 0);
            self.buf.edit_at_cursor(c, past_cursors, &text);
            c.select_range(s);
        });
    }

    pub fn duplicate_lines_down(&mut self) {
        self.cursors.foreach(|c, past_cursors| {
            let s = c.selection();

            c.select_overlapped_lines();
            let mut text = self.buf.substr(c.selection());
            let num_lines = text.trim_end().matches('\n').count() + 1;
            if !text.ends_with('\n') {
                text.push('\n');
            }

            c.move_to(c.front().y, 0);
            self.buf.edit_at_cursor(c, past_cursors, &text);
            c.select(
                s.start.y + num_lines,
                s.start.x,
                s.end.y + num_lines,
                s.end.x,
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::cursor::Cursor;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn duplicate_a_line_up() {
        let mut b = Buffer::from_text("");
        b.set_cursors_for_test(&[Cursor::new(0, 0)]);
        b.duplicate_lines_up();
        assert_eq!(b.text(), "\n");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        let mut b = Buffer::from_text("abcd");
        b.set_cursors_for_test(&[Cursor::new(0, 2)]);
        b.duplicate_lines_up();
        assert_eq!(b.text(), "abcd\nabcd");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);

        let mut b = Buffer::from_text("abcd\nxyz");
        b.set_cursors_for_test(&[Cursor::new(1, 2)]);
        b.duplicate_lines_up();
        assert_eq!(b.text(), "abcd\nxyz\nxyz");
        assert_eq!(b.cursors(), &[Cursor::new(1, 2)]);
    }

    #[test]
    fn duplicate_multiple_lines_up() {
        // ABCD
        // EFGH
        // ----
        let mut b = Buffer::from_text("ABCD\nEFGH\n----");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 1, 1, 0)]);
        b.duplicate_lines_up();
        assert_eq!(b.text(), "ABCD\nABCD\nEFGH\n----");
        assert_eq!(b.cursors(), &[Cursor::new_selection(0, 1, 1, 0)]);

        // ABCD
        // EFGH
        // ----
        let mut b = Buffer::from_text("ABCD\nEFGH\n----");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 1, 2, 4)]);
        b.duplicate_lines_up();
        assert_eq!(b.text(), "ABCD\nEFGH\n----\nABCD\nEFGH\n----");
        assert_eq!(b.cursors(), &[Cursor::new_selection(0, 1, 2, 4)]);
    }

    #[test]
    fn duplicate_a_line_down() {
        let mut b = Buffer::from_text("");
        b.set_cursors_for_test(&[Cursor::new(0, 0)]);
        b.duplicate_lines_down();
        assert_eq!(b.text(), "\n");
        assert_eq!(b.cursors(), &[Cursor::new(1, 0)]);

        let mut b = Buffer::from_text("abcd");
        b.set_cursors_for_test(&[Cursor::new(0, 2)]);
        b.duplicate_lines_down();
        assert_eq!(b.text(), "abcd\nabcd");
        assert_eq!(b.cursors(), &[Cursor::new(1, 2)]);

        let mut b = Buffer::from_text("abcd\nxyz");
        b.set_cursors_for_test(&[Cursor::new(1, 2)]);
        b.duplicate_lines_down();
        assert_eq!(b.text(), "abcd\nxyz\nxyz");
        assert_eq!(b.cursors(), &[Cursor::new(2, 2)]);
    }

    #[test]
    fn duplicate_multiple_lines_down() {
        // ABCD
        // EFGH
        // ----
        let mut b = Buffer::from_text("ABCD\nEFGH\n----");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 1, 1, 0)]);
        b.duplicate_lines_down();
        assert_eq!(b.text(), "ABCD\nABCD\nEFGH\n----");
        assert_eq!(b.cursors(), &[Cursor::new_selection(1, 1, 2, 0)]);

        // ABCD
        // EFGH
        // ----
        let mut b = Buffer::from_text("ABCD\nEFGH\n----");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 1, 2, 4)]);
        b.duplicate_lines_down();
        assert_eq!(b.text(), "ABCD\nEFGH\n----\nABCD\nEFGH\n----");
        assert_eq!(b.cursors(), &[Cursor::new_selection(3, 1, 5, 4)]);
    }
}
