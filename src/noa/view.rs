use std::cmp::{self, min};

use std::ops;

use noa_buffer::{Buffer, Cursor, Point, Range};

use crate::terminal::display_width::DisplayWidth;

#[derive(Debug, Clone)]
pub struct DisplayLine {
    /// The char indices in a line rope.
    pub chunks: Vec<ops::Range<usize>>,
    /// The char indices in the whole buffer rope.
    pub range: Range,
}

pub struct View {
    lines: Vec<DisplayLine>,
    /// The index of display line.
    top_left: usize,
    height: usize,
}

impl View {
    pub fn new() -> View {
        View {
            lines: Vec::new(),
            top_left: 0,
            height: 0,
        }
    }

    pub fn adjust_top_left(&mut self, main_cursor_pos: &Point) {
        let index = self.point_to_display_line(main_cursor_pos).unwrap();
        if index < self.top_left {
            self.top_left = index;
        }

        if index >= self.top_left + self.height {
            self.top_left = index - self.height + 1;
        }
    }

    /// Returns `(screen_y, screen_x)`.
    pub fn point_to_display_pos(
        &self,
        pos: &Point,
        screen_y_end: usize,
        screen_text_start: usize,
        buffer_num_lines: usize,
    ) -> (usize, usize) {
        self.point_to_display_line(pos)
            .map(|i| {
                let display_line = &self.lines[i];
                (
                    i - self.top_left,
                    screen_text_start + pos.x - display_line.range.front().x,
                )
            })
            .unwrap_or_else(|| {
                if pos.y == buffer_num_lines && pos.x == 0 {
                    // EOF.
                    return (screen_y_end, screen_text_start);
                }

                panic!("failed to determine the pos in the view: {}", pos);
            })
    }

    fn point_to_display_line(&self, pos: &Point) -> Option<usize> {
        self.lines
            .binary_search_by(|line| {
                if line.range.contains(pos) || line.range.back() == pos {
                    cmp::Ordering::Equal
                } else if pos < line.range.front() {
                    cmp::Ordering::Greater
                } else {
                    cmp::Ordering::Less
                }
            })
            .ok()
    }

    pub fn visible_display_lines(&self) -> &[DisplayLine] {
        &self.lines[self.top_left..min(self.lines.len(), self.top_left + self.height)]
    }

    pub fn layout(&mut self, buffer: &Buffer, y_from: usize, width: usize, height: usize) {
        self.height = height;
        if y_from == 0 {
            self.lines.clear();
        } else {
            self.lines
                .truncate(self.point_to_display_line(&Point::new(y_from, 0)).unwrap());
        }

        for text_y in y_from..buffer.num_lines() {
            let line_rope = buffer.line(text_y);
            let mut spans = Vec::new();
            let mut width_remaining = width;
            let mut text_x = 0;
            let mut front = Point::new(text_y, text_x);

            if line_rope.len_chars() == 0 {
                self.lines.push(DisplayLine {
                    chunks: vec![],
                    range: Range::from_points(Point::new(text_y, 0), Point::new(text_y, 0)),
                });
            } else {
                for mut chunk in line_rope.chunks() {
                    let chunk_width = chunk.display_width();
                    if chunk_width <= width_remaining {
                        spans.push(text_x..(text_x + chunk_width));
                        text_x += chunk_width;
                        width_remaining -= chunk_width;
                    } else {
                        // Needs a soft wrap.
                        let _i = 0;
                        while !chunk.is_empty() {
                            let mut wrap_byte_at = 0;
                            let mut wrap_char_at = 0;
                            for (i, ch) in chunk.char_indices() {
                                if ch.display_width() > width_remaining {
                                    break;
                                }

                                wrap_byte_at = i + ch.len_utf8();
                                wrap_char_at += 1;
                                width_remaining -= ch.display_width();
                            }

                            spans.push(text_x..(text_x + wrap_byte_at));

                            text_x += wrap_char_at;
                            self.lines.push(DisplayLine {
                                chunks: spans,
                                range: Range::from_points(front, Point::new(text_y, text_x)),
                            });

                            spans = Vec::new();
                            chunk = &chunk[wrap_byte_at..];
                            front = Point::new(text_y, text_x);
                            width_remaining = width;
                        }
                    }
                }

                if front.x != text_x {
                    self.lines.push(DisplayLine {
                        chunks: spans,
                        range: Range::from_points(front, Point::new(text_y, text_x)),
                    });
                }
            }
        }
    }

    pub fn move_cursors(&self, buffer: &mut Buffer, y_diff: isize, x_diff: isize) {
        let mut new_cursors = Vec::new();
        for cursor in buffer.cursors() {
            // Cancel the selection.
            let pos = match cursor {
                Cursor::Normal { pos, .. } => pos,
                Cursor::Selection(range) => &range.end,
            };

            // Move the cursor.
            let new_pos = self.move_x(&self.move_y(&pos, y_diff), x_diff);
            new_cursors.push(Cursor::Normal {
                pos: new_pos,
                logical_x: new_pos.x,
            });
        }

        buffer.set_cursors(new_cursors);
    }

    pub fn expand_selections(&self, buffer: &mut Buffer, y_diff: isize, x_diff: isize) {
        let mut new_cursors = Vec::new();
        for cursor in buffer.cursors() {
            let (start, end) = match cursor {
                Cursor::Normal { pos, .. } => (pos, pos),
                Cursor::Selection(range) => (&range.start, &range.end),
            };

            // Move the cursor.
            let new_end = self.move_x(&self.move_y(&end, y_diff), x_diff);
            new_cursors.push(Cursor::Selection(Range::from_points(*start, new_end)));
        }

        buffer.set_cursors(new_cursors);
    }

    fn move_y(&self, pos: &Point, y_diff: isize) -> Point {
        let prev_y = self.point_to_display_line(pos).unwrap();
        let prev_line = &self.lines[prev_y];

        let new_y = if y_diff < 0 {
            prev_y.saturating_sub(y_diff.abs() as usize)
        } else {
            prev_y + y_diff.abs() as usize
        };

        let &new_line = &self
            .lines
            .get(new_y)
            .unwrap_or_else(|| &self.lines[self.lines.len() - 1]);

        let char_x = pos.x - prev_line.range.front().x;

        Point::new(
            new_line.range.front().y,
            min(new_line.range.front().x + char_x, new_line.range.back().x),
        )
    }

    fn move_x(&self, pos: &Point, x_diff: isize) -> Point {
        let current_y = self.point_to_display_line(pos).unwrap();
        let current_line = &self.lines[current_y];
        let mut new_pos = *pos;

        if x_diff > 0 {
            assert!(x_diff == 1);
            let new_x = pos.x + 1;
            if new_x < current_line.range.back().x {
                new_pos.x = new_x;
            } else if let Some(next_line) = self.lines.get(current_y + 1) {
                new_pos = *next_line.range.front();
            }
        } else if x_diff == -1 {
            if pos.x > 0 && pos.x > current_line.range.front().x {
                new_pos.x = pos.x - 1;
            } else if current_y > 0 {
                if let Some(prev_line) = self.lines.get(current_y - 1) {
                    new_pos = *prev_line.range.back();
                }
            }
        }

        new_pos
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn layout_without_softwrap() {
        let mut view = View::new();
        let buffer = Buffer::from_str("123\nabc\n\nxyz");
        view.layout(&buffer, 0, 5, 3);
        assert_eq!(view.lines.len(), 4);
        assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 3));
        assert_eq!(view.lines[1].range, Range::new(1, 0, 1, 3));
        assert_eq!(view.lines[2].range, Range::new(2, 0, 2, 0));
        assert_eq!(view.lines[3].range, Range::new(3, 0, 3, 3));
    }

    #[test]
    fn layout_newlines() {
        let mut view = View::new();
        let buffer = Buffer::from_str("\n\n\n");
        view.layout(&buffer, 0, 5, 3);
        assert_eq!(view.lines.len(), 4);
        assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 0));
        assert_eq!(view.lines[1].range, Range::new(1, 0, 1, 0));
        assert_eq!(view.lines[2].range, Range::new(2, 0, 2, 0));
        assert_eq!(view.lines[3].range, Range::new(3, 0, 3, 0));
    }

    #[test]
    fn layout_with_softwrap1() {
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abc\nxyz");
        view.layout(&buffer, 0, 5, 3);
        assert_eq!(view.lines.len(), 3);
        assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 5));
        assert_eq!(view.lines[1].range, Range::new(0, 5, 0, 8));
        assert_eq!(view.lines[2].range, Range::new(1, 0, 1, 3));
    }

    #[test]
    fn layout_with_softwrap2() {
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#$%\nxyz\nLMNO");
        view.layout(&buffer, 0, 5, 3);
        assert_eq!(view.lines.len(), 5);
        assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 5));
        assert_eq!(view.lines[1].range, Range::new(0, 5, 0, 10));
        assert_eq!(view.lines[2].range, Range::new(0, 10, 0, 15));
        assert_eq!(view.lines[3].range, Range::new(1, 0, 1, 3));
        assert_eq!(view.lines[4].range, Range::new(2, 0, 2, 4));

        view.layout(&buffer, 1, 5, 3);
        assert_eq!(view.lines.len(), 5);
        assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 5));
        assert_eq!(view.lines[1].range, Range::new(0, 5, 0, 10));
        assert_eq!(view.lines[2].range, Range::new(0, 10, 0, 15));
        assert_eq!(view.lines[3].range, Range::new(1, 0, 1, 3));
        assert_eq!(view.lines[4].range, Range::new(2, 0, 2, 4));
    }

    #[test]
    fn point_to_display_line() {
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#$%\nxyz");
        view.layout(&buffer, 0, 5, 3);
        assert_eq!(view.point_to_display_line(&Point::new(0, 0)), Some(0));
        assert_eq!(view.point_to_display_line(&Point::new(0, 5)), Some(1));
        assert_eq!(view.point_to_display_line(&Point::new(0, 14)), Some(2));
        assert_eq!(view.point_to_display_line(&Point::new(0, 15)), Some(2));
        assert_eq!(view.point_to_display_line(&Point::new(1, 16)), None);
        assert_eq!(view.point_to_display_line(&Point::new(1, 2)), Some(3));
        assert_eq!(view.point_to_display_line(&Point::new(1, 3)), Some(3));
        assert_eq!(view.point_to_display_line(&Point::new(1, 4)), None);
    }

    #[test]
    fn move_x() {
        // 12345
        // abcde
        // !@#
        // xyz
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#\nxyz");
        view.layout(&buffer, 0, 5, 3);
        assert_eq!(
            // 1|2345
            view.move_x(&Point::new(0, 1), 1),
            // 12|345
            Point::new(0, 2)
        );
        assert_eq!(
            // 1234|5
            view.move_x(&Point::new(0, 4), 1),
            // both 12345| and |abcde
            Point::new(0, 5)
        );
        assert_eq!(
            // both 12345| and |abcde
            view.move_x(&Point::new(0, 5), 1),
            // a|bcde
            Point::new(0, 6)
        );
        assert_eq!(
            // !@#|
            view.move_x(&Point::new(0, 13), 1),
            // |xyz
            Point::new(1, 0)
        );

        assert_eq!(
            // 12|345
            view.move_x(&Point::new(0, 2), -1),
            // 1|2345
            Point::new(0, 1)
        );
        assert_eq!(
            // |xyz
            view.move_x(&Point::new(1, 0), -1),
            // !@#|
            Point::new(0, 13)
        );

        assert_eq!(
            // |12345
            view.move_x(&Point::new(0, 0), -1),
            // |12345
            Point::new(0, 0)
        );
        assert_eq!(
            // xyz|
            view.move_x(&Point::new(1, 3), 1),
            // xyz|
            Point::new(1, 3)
        );
    }

    #[test]
    fn move_y() {
        // 12345
        // abcde
        // !@#
        // xyz
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#$\nxyz");
        view.layout(&buffer, 0, 5, 3);
        assert_eq!(
            // 1|2345
            view.move_y(&Point::new(0, 1), 1),
            // a|bcde
            Point::new(0, 6)
        );
        assert_eq!(
            // a|bcde
            view.move_y(&Point::new(0, 6), 1),
            // !|@#
            Point::new(0, 11)
        );
        assert_eq!(
            // !|@#
            view.move_y(&Point::new(0, 11), 1),
            // x|yz
            Point::new(1, 1)
        );

        assert_eq!(
            // x|yz
            view.move_y(&Point::new(1, 1), -1),
            // !|@#
            Point::new(0, 11)
        );
        assert_eq!(
            // !|@#
            view.move_y(&Point::new(0, 11), -1),
            // a|bcde
            Point::new(0, 6)
        );
        assert_eq!(
            // !|@#
            view.move_y(&Point::new(0, 6), -1),
            // a|bcde
            Point::new(0, 1)
        );

        assert_eq!(
            // 1|2345
            view.move_y(&Point::new(0, 1), -1),
            // 1|2345
            Point::new(0, 1)
        );
        assert_eq!(
            // x|yz
            view.move_y(&Point::new(1, 1), 1),
            // x|yz
            Point::new(1, 1)
        );
    }

    #[test]
    fn adjust_top_left() {
        // 12345|
        // abcde|
        // -----+
        // !@#
        // xyz
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#$\nxyz");
        view.layout(&buffer, 0, 5, 2);
        view.adjust_top_left(&Point::new(0, 6));
        assert_eq!(view.top_left, 0);

        // 12345
        // abcde
        // -----+
        // !@#  |
        // xyz  |
        view.adjust_top_left(&Point::new(1, 0));
        assert_eq!(view.top_left, 2);

        // 12345
        // -----+
        // abcde|
        // !@#  |
        // xyz
        view.adjust_top_left(&Point::new(0, 6));
        assert_eq!(view.top_left, 1);
    }
}
