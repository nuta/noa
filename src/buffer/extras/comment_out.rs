use std::{cmp::min, collections::HashMap};

use crate::{
    buffer::Buffer,
    cursor::{Position, Range},
};

impl Buffer {
    pub fn toggle_line_comment_out(&mut self) {
        let keyword_without_whitespace = match self.language().line_comment.as_ref() {
            Some(keyword) => *keyword,
            None => return,
        };
        let keyword_with_whitespace = format!("{} ", keyword_without_whitespace);
        let keyword_without_whitespace_len = keyword_without_whitespace.chars().count();
        let keyword_with_whitespace_len = keyword_with_whitespace.chars().count();

        let mut target_lines = Vec::new();
        for c in self.cursors() {
            let ys = c.selection().overlapped_lines();
            if ys.is_empty() {
                target_lines.push(c.front().y);
            } else {
                for y in ys {
                    target_lines.push(y);
                }
            }
        }

        let increment_comment = target_lines.iter().any(|y| {
            !self
                .buf
                .line_text(*y)
                .trim_start()
                .starts_with(keyword_without_whitespace)
        });

        // Add/remove comment outs.
        let mut x_diffs = HashMap::new();
        for y in target_lines {
            let current_indent_len = self.buf.line_indent_len(y);
            let pos_after_indent = Position::new(y, current_indent_len);
            let eol = Position::new(y, self.buf.line_len(y));
            let stripped_line_text = self.substr(Range::from_positions(pos_after_indent, eol));

            if increment_comment {
                self.buf.edit(
                    Range::from_positions(pos_after_indent, pos_after_indent),
                    &keyword_with_whitespace,
                );
            } else {
                if stripped_line_text.starts_with(&keyword_with_whitespace) {
                    let end = Position::new(y, current_indent_len + keyword_with_whitespace_len);
                    self.buf
                        .edit(Range::from_positions(pos_after_indent, end), "");
                    x_diffs.insert(y, keyword_with_whitespace_len);
                } else if stripped_line_text.starts_with(keyword_without_whitespace) {
                    let end = Position::new(y, current_indent_len + keyword_without_whitespace_len);
                    self.buf
                        .edit(Range::from_positions(pos_after_indent, end), "");
                    x_diffs.insert(y, keyword_without_whitespace_len);
                }
            }
        }

        // Adjust cursors.
        self.cursors.foreach(|c, _| {
            let range = c.selection_mut();
            let x_diff = x_diffs.get(&range.start.y).copied().unwrap_or(0);
            if increment_comment {
                range.start.x = min(
                    range.start.x.saturating_sub(x_diff),
                    self.buf.line_len(range.start.y),
                );
                range.end.x = min(
                    range.end.x.saturating_sub(x_diff),
                    self.buf.line_len(range.end.y),
                );
            } else {
                range.start.x = min(range.start.x + (x_diff), self.buf.line_len(range.start.y));
                range.end.x = min(range.end.x + (x_diff), self.buf.line_len(range.end.y));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::cursor::Cursor;
    use noa_languages::get_language_by_name;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_comment_out() {
        let lang = get_language_by_name("rust").unwrap();

        let mut buffer = Buffer::from_text("");
        buffer.set_language(lang).unwrap();
        buffer.set_cursors_for_test(&[Cursor::new(0, 0)]);
        buffer.toggle_line_comment_out();
        assert_eq!(buffer.text(), "// ");

        let mut buffer = Buffer::from_text("abc");
        buffer.set_language(lang).unwrap();
        buffer.set_cursors_for_test(&[Cursor::new(0, 0)]);
        buffer.toggle_line_comment_out();
        assert_eq!(buffer.text(), "// abc");

        let mut buffer = Buffer::from_text("    abc");
        buffer.set_language(lang).unwrap();
        buffer.set_cursors_for_test(&[Cursor::new(0, 0)]);
        buffer.toggle_line_comment_out();
        assert_eq!(buffer.text(), "    // abc");

        let mut buffer = Buffer::from_text("  abc\n  def");
        buffer.set_language(lang).unwrap();
        buffer.set_cursors_for_test(&[Cursor::new_selection(0, 0, 2, 0)]);
        buffer.toggle_line_comment_out();
        assert_eq!(buffer.text(), "  // abc\n  // def");

        let mut buffer = Buffer::from_text("  abc\n  // def");
        buffer.set_language(lang).unwrap();
        buffer.set_cursors_for_test(&[Cursor::new_selection(0, 0, 2, 0)]);
        buffer.toggle_line_comment_out();
        assert_eq!(buffer.text(), "  // abc\n  // // def");
    }

    #[test]
    fn test_uncomment_out() {
        let lang = get_language_by_name("rust").unwrap();

        let mut buffer = Buffer::from_text("//");
        buffer.set_language(lang).unwrap();
        buffer.set_cursors_for_test(&[Cursor::new(0, 0)]);
        buffer.toggle_line_comment_out();
        assert_eq!(buffer.text(), "");

        let mut buffer = Buffer::from_text("// abc");
        buffer.set_language(lang).unwrap();
        buffer.set_cursors_for_test(&[Cursor::new(0, 6)]);
        buffer.toggle_line_comment_out();
        assert_eq!(buffer.text(), "abc");
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 3)]);

        let mut buffer = Buffer::from_text("    // abc");
        buffer.set_language(lang).unwrap();
        buffer.set_cursors_for_test(&[Cursor::new(0, 10)]);
        buffer.toggle_line_comment_out();
        assert_eq!(buffer.text(), "    abc");
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 7)]);

        let mut buffer = Buffer::from_text("  // abc\n  // def");
        buffer.set_language(lang).unwrap();
        buffer.set_cursors_for_test(&[Cursor::new_selection(0, 0, 2, 0)]);
        buffer.toggle_line_comment_out();
        assert_eq!(buffer.text(), "  abc\n  def");
    }
}
