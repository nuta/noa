use std::{cmp::min, collections::HashMap};

use noa_editorconfig::{EditorConfig, IndentStyle};

use crate::{
    buffer::Buffer,
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
};

pub(crate) fn compute_desired_indent_len(
    buf: &RawBuffer,
    config: &EditorConfig,
    y: usize,
) -> usize {
    let (prev_indent_len, char_at_cursor) = if y == 0 {
        (0, None)
    } else {
        let prev_indent_len = buf.line_indent_len(y - 1);
        let pos_at_newline = Position::new(y - 1, buf.line_len(y - 1));
        let char_at_cursor = buf.char_iter(pos_at_newline).prev();
        (prev_indent_len, char_at_cursor)
    };

    match char_at_cursor {
        Some('{') => prev_indent_len + config.indent_size,
        _ => prev_indent_len,
    }
}

impl Buffer {
    pub fn indent(&mut self) {
        if let Some(cursor) = self.cursors.single_selection_cursor() {
            let ys = cursor.selection().overlapped_lines();
            if !ys.is_empty() {
                for y in ys {
                    let desired_len = compute_desired_indent_len(&self.buf, &self.config, y);
                    let current_indent_len = self.buf.line_indent_len(y);

                    let indent_size = if desired_len <= current_indent_len {
                        self.config.indent_size
                    } else {
                        desired_len - current_indent_len
                    };

                    let indent_str = match self.config.indent_style {
                        IndentStyle::Tab => "\t".repeat(indent_size),
                        IndentStyle::Space => " ".repeat(indent_size),
                    };

                    self.buf.edit(Range::new(y, 0, y, 0), &indent_str);
                }

                return;
            }
        }

        // How many indentation characters should we add for each cursors?
        let mut increase_lens = Vec::new();
        for c in &self.cursors {
            let pos = c.front();

            let desired_len = compute_desired_indent_len(&self.buf, &self.config, pos.y);
            let current_indent_len = self.buf.line_indent_len(pos.y);
            let n = if pos.x < desired_len && pos.x == current_indent_len {
                desired_len - pos.x
            } else {
                let mut x = pos.x + 1;
                while x % self.config.indent_size != 0 {
                    x += 1;
                }
                x - pos.x
            };

            increase_lens.push(n);
        }

        // Insert indentations.
        let mut increase_lens_iter = increase_lens.iter();
        self.cursors.foreach(|c, past_cursors| {
            let indent_size = *increase_lens_iter.next().unwrap();
            self.buf.edit_at_cursor(
                c,
                past_cursors,
                &match self.config.indent_style {
                    IndentStyle::Tab => "\t".repeat(indent_size),
                    IndentStyle::Space => " ".repeat(indent_size),
                },
            );
        });
    }

    pub fn deindent(&mut self) {
        let mut ys = Vec::new();
        if let Some(cursor) = self.cursors.single_selection_cursor() {
            let range = cursor.selection().overlapped_lines();
            if !range.is_empty() {
                ys.push(range);
            }
        }

        if ys.is_empty() {
            for c in &self.cursors {
                let y = c.front().y;
                ys.push(y..(y + 1));
            }
        }

        let mut deindented_sizes = HashMap::new();
        for range in ys {
            for y in range {
                if deindented_sizes.contains_key(&y) {
                    continue;
                }

                let n = min(self.config.indent_size, self.buf.line_indent_len(y));
                self.buf.edit(Range::new(y, 0, y, n), "");
                deindented_sizes.insert(y, n);
            }
        }

        self.cursors.foreach(|c, _| {
            let range = c.selection_mut();
            range.start.x = min(
                range
                    .start
                    .x
                    .saturating_sub(deindented_sizes.get(&range.start.y).copied().unwrap_or(0)),
                self.buf.line_len(range.start.y),
            );
            range.end.x = min(
                range
                    .end
                    .x
                    .saturating_sub(deindented_sizes.get(&range.end.y).copied().unwrap_or(0)),
                self.buf.line_len(range.end.y),
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::cursor::Cursor;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_indent() {
        let mut buffer = Buffer::from_text("");
        buffer.set_cursors_for_test(&[Cursor::new(0, 0)]);
        buffer.indent();
        assert_eq!(buffer.text(), "    ");

        let mut buffer = Buffer::from_text("abc");
        buffer.set_cursors_for_test(&[Cursor::new(0, 0)]);
        buffer.indent();
        assert_eq!(buffer.text(), "    abc");
    }

    #[test]
    fn test_indent_selection() {
        let mut buffer = Buffer::from_text("abc");
        buffer.set_cursors_for_test(&[Cursor::new_selection(0, 1, 0, 2)]);
        buffer.indent();
        assert_eq!(buffer.text(), "a   c");

        // A
        // B
        // C
        let mut buffer = Buffer::from_text("A\nB\nC\n");
        buffer.set_cursors_for_test(&[Cursor::new_selection(0, 0, 2, 0)]);
        buffer.indent();
        assert_eq!(buffer.text(), "    A\n    B\nC\n");
        buffer.indent();
        assert_eq!(buffer.text(), "        A\n        B\nC\n");
    }

    #[test]
    fn test_deindent() {
        let mut buffer = Buffer::from_text("");
        buffer.set_cursors_for_test(&[Cursor::new(0, 0)]);
        buffer.deindent();
        assert_eq!(buffer.text(), "");

        let mut buffer = Buffer::from_text("    ");
        buffer.set_cursors_for_test(&[Cursor::new(0, 0)]);
        buffer.deindent();
        assert_eq!(buffer.text(), "");

        let mut buffer = Buffer::from_text("        abc");
        buffer.set_cursors_for_test(&[Cursor::new(0, 0)]);
        buffer.deindent();
        assert_eq!(buffer.text(), "    abc");
    }

    #[test]
    fn test_deindent_selection() {
        let mut buffer = Buffer::from_text("    abc");
        buffer.set_cursors_for_test(&[Cursor::new_selection(0, 5, 0, 6)]);
        buffer.deindent();
        assert_eq!(buffer.text(), "abc");

        let mut buffer = Buffer::from_text("    A\n        B\n    C\n");
        buffer.set_cursors_for_test(&[Cursor::new_selection(0, 0, 2, 0)]);
        buffer.deindent();
        assert_eq!(buffer.text(), "A\n    B\n    C\n");
        buffer.deindent();
        assert_eq!(buffer.text(), "A\nB\n    C\n");
    }
}
