use noa_editorconfig::{EditorConfig, IndentStyle};

use crate::{buffer::Buffer, cursor::Position, raw_buffer::RawBuffer};

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
        self.cursors.foreach(|_c, _past_cursors| {
            // let n = min(
            //     self.buf
            //         .char(Position::new(y, 0))
            //         .take_while(|c| *c == ' ' || *c == '\t')
            //         .count(),
            //     self.config.indent_size,
            // );
            // self.buf.edit_cursor(Range::new(y, 0, y, n), "")
            todo!()
        });
    }
}
