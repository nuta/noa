use crate::{buffer::Buffer, cursor::Range};

impl Buffer {
    pub fn move_lines_up(&mut self) {
        self.cursors.foreach(|c, past_cursors| {
            if c.front().y == 0 {
                return;
            }

            let s = c.selection();

            c.select_overlapped_lines();
            let mut text = self.buf.substr(c.selection());
            let should_trim = !text.ends_with('\n');
            if !text.ends_with('\n') {
                text.push('\n');
            }

            let prev_line = self
                .buf
                .substr(Range::new(c.front().y - 1, 0, c.front().y, 0));
            text.push_str(&prev_line);
            if should_trim && text.ends_with('\n') {
                text.pop();
            }

            c.select(s.front().y - 1, 0, s.back().y + 1, 0);
            self.buf.edit_at_cursor(c, past_cursors, &text);
            c.select(s.start.y - 1, s.start.x, s.end.y - 1, s.end.x);
        });
    }

    pub fn move_lines_down(&mut self) {
        self.cursors.foreach(|c, past_cursors| {
            if c.back().y >= self.buf.num_lines() - 1 {
                return;
            }

            let s = c.selection();

            c.select_overlapped_lines();
            let mut text = self.buf.substr(c.selection());

            let mut next_line = self
                .buf
                .substr(Range::new(c.back().y, 0, c.back().y + 1, 0));
            let should_trim = !next_line.ends_with('\n');
            if !next_line.ends_with('\n') {
                next_line.push('\n');
            }

            text.insert_str(0, &next_line);
            if should_trim && text.ends_with('\n') {
                text.pop();
            }

            c.select(c.front().y, 0, c.back().y + 1, 0);
            self.buf.edit_at_cursor(c, past_cursors, &text);
            c.select(s.start.y + 1, s.start.x, s.end.y + 1, s.end.x);
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::cursor::Cursor;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn move_a_line_up() {
        let mut b = Buffer::from_text("");
        b.set_cursors_for_test(&[Cursor::new(0, 0)]);
        b.move_lines_up();
        assert_eq!(b.text(), "");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        // abcd
        let mut b = Buffer::from_text("abcd");
        b.set_cursors_for_test(&[Cursor::new(0, 2)]);
        b.move_lines_up();
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);

        //
        // abcd
        let mut b = Buffer::from_text("\nabcd");
        b.set_cursors_for_test(&[Cursor::new(1, 2)]);
        b.move_lines_up();
        assert_eq!(b.text(), "abcd\n");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);

        // abcd
        // xyz
        let mut b = Buffer::from_text("abcd\nxyz");
        b.set_cursors_for_test(&[Cursor::new(1, 2)]);
        b.move_lines_up();
        assert_eq!(b.text(), "xyz\nabcd");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);

        // abcd
        // xyz
        //
        let mut b = Buffer::from_text("abcd\nxyz\n");
        b.set_cursors_for_test(&[Cursor::new(1, 2)]);
        b.move_lines_up();
        assert_eq!(b.text(), "xyz\nabcd\n");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);
    }

    #[test]
    fn move_multiple_lines_up() {
        // abcd
        // efgh
        // xyz
        //
        let mut b = Buffer::from_text("abcd\nefgh\nxyz\n");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 2, 2, 1)]);
        b.move_lines_up();
        assert_eq!(b.text(), "abcd\nefgh\nxyz\n");
        assert_eq!(b.cursors(), &[Cursor::new_selection(0, 2, 2, 1)]);

        //
        // abcd
        // efgh
        // xyz
        //
        let mut b = Buffer::from_text("\nabcd\nefgh\nxyz\n");
        b.set_cursors_for_test(&[Cursor::new_selection(1, 2, 3, 1)]);
        b.move_lines_up();
        assert_eq!(b.text(), "abcd\nefgh\nxyz\n\n");
        assert_eq!(b.cursors(), &[Cursor::new_selection(0, 2, 2, 1)]);

        // ----
        // abcd
        // xyz
        let mut b = Buffer::from_text("----\nabcd\nxyz");
        b.set_cursors_for_test(&[Cursor::new_selection(1, 2, 2, 1)]);
        b.move_lines_up();
        assert_eq!(b.text(), "abcd\nxyz\n----");
        assert_eq!(b.cursors(), &[Cursor::new_selection(0, 2, 1, 1)]);
    }

    #[test]
    fn move_a_line_down() {
        let mut b = Buffer::from_text("");
        b.set_cursors_for_test(&[Cursor::new(0, 0)]);
        b.move_lines_down();
        assert_eq!(b.text(), "");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        // abcd
        let mut b = Buffer::from_text("abcd");
        b.set_cursors_for_test(&[Cursor::new(0, 2)]);
        b.move_lines_down();
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);

        // abcd
        //
        let mut b = Buffer::from_text("abcd\n");
        b.set_cursors_for_test(&[Cursor::new(0, 2)]);
        b.move_lines_down();
        assert_eq!(b.text(), "\nabcd");
        assert_eq!(b.cursors(), &[Cursor::new(1, 2)]);

        // abcd
        // xyz
        let mut b = Buffer::from_text("abcd\nxyz");
        b.set_cursors_for_test(&[Cursor::new(0, 2)]);
        b.move_lines_down();
        assert_eq!(b.text(), "xyz\nabcd");
        assert_eq!(b.cursors(), &[Cursor::new(1, 2)]);

        // abcd
        // xyz
        //
        let mut b = Buffer::from_text("abcd\nxyz\n");
        b.set_cursors_for_test(&[Cursor::new(1, 2)]);
        b.move_lines_up();
        assert_eq!(b.text(), "xyz\nabcd\n");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);
    }

    #[test]
    fn move_multiple_lines_down() {
        //
        // abcd
        // efgh
        // xyz
        let mut b = Buffer::from_text("\nabcd\nefgh\nxyz");
        b.set_cursors_for_test(&[Cursor::new_selection(1, 2, 3, 1)]);
        b.move_lines_down();
        assert_eq!(b.text(), "\nabcd\nefgh\nxyz");
        assert_eq!(b.cursors(), &[Cursor::new_selection(1, 2, 3, 1)]);

        //
        // abcd
        // efgh
        // xyz
        //
        let mut b = Buffer::from_text("\nabcd\nefgh\nxyz\n");
        b.set_cursors_for_test(&[Cursor::new_selection(1, 2, 3, 1)]);
        b.move_lines_down();
        assert_eq!(b.text(), "\n\nabcd\nefgh\nxyz");
        assert_eq!(b.cursors(), &[Cursor::new_selection(2, 2, 3, 1)]);

        // abcd
        // xyz
        // ----
        let mut b = Buffer::from_text("abcd\nxyz\n----");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 2, 1, 1)]);
        b.move_lines_down();
        assert_eq!(b.text(), "----\nabcd\nxyz");
        assert_eq!(b.cursors(), &[Cursor::new_selection(1, 2, 2, 1)]);
    }
}
