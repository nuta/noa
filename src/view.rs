use std::cmp::{self, min};
use std::convert::TryInto;
use std::ops;

use crate::{buffer::Buffer, rope::Point, rope::Range, terminal::DisplayWidth};

#[derive(Debug, Copy, Clone)]
pub enum Style {
    Cursor,
}

#[derive(Debug, Clone)]
pub enum Span {
    Text {
        /// The char indices in a line rope.
        char_range: ops::Range<usize>,
    },
    Style(Style),
}

#[derive(Debug, Clone)]
pub struct DisplayLine {
    spans: Vec<Span>,
    range: Range,
}

pub struct View {
    lines: Vec<DisplayLine>,
    /// The index of display line.
    top_left: usize,
}

impl View {
    pub fn new() -> View {
        View {
            lines: Vec::new(),
            top_left: 0,
        }
    }

    pub fn compute_new_cursor_pos(&mut self, pos: &Point, y_diff: isize, x_diff: isize) -> Point {
        let prev_y = self.point_to_display_line(pos).unwrap();
        let prev_line = &self.lines[prev_y];

        let (current_line, mut new_pos) = if y_diff.abs() > 0 {
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

            (
                new_line,
                Point::new(
                    new_line.range.front().y,
                    min(new_line.range.front().x + char_x, new_line.range.back().x),
                ),
            )
        } else {
            (prev_line, *pos)
        };

        if x_diff.abs() > 0 {
            assert!(x_diff == 1);
            let new_x = new_pos.x + x_diff.abs() as usize;
            if x_diff > 0 {
                if new_x <= current_line.range.back().x {
                    new_pos.x = new_x;
                } else {
                    new_pos.y += 1;
                    new_pos.x = 0;
                }
            } else {
                new_pos.y = new_pos.y.saturating_sub(1);
                new_pos.x = self.lines[new_pos.y].range.back().x;
            }
        }

        new_pos
    }

    pub fn adjust_top_left(&mut self, main_cursor_pos: &Point, height: usize) {
        let index = self.point_to_display_line(main_cursor_pos).unwrap();
        if index < self.top_left {
            self.top_left = index;
        }

        if index >= self.top_left + height {
            self.top_left = index - height + 1;
        }
    }

    fn point_to_display_line(&self, pos: &Point) -> Option<usize> {
        self.lines
            .binary_search_by(|line| {
                if line.range.contains(pos) {
                    cmp::Ordering::Equal
                } else if pos < line.range.front() {
                    cmp::Ordering::Greater
                } else {
                    cmp::Ordering::Less
                }
            })
            .ok()
    }

    pub fn layout(&mut self, buffer: &Buffer, width: usize, height: usize) {
        for text_y in 0..buffer.num_lines() {
            let mut line_rope = buffer.line(text_y);
            let mut spans = Vec::new();
            let mut width_remaining = width;
            let mut text_x = 0;
            let mut front = Point::new(text_y, text_x);

            if line_rope.len_chars() == 0 {
                self.lines.push(DisplayLine {
                    spans: vec![],
                    range: Range::from_points(Point::new(text_y, 0), Point::new(text_y, 0)),
                });
            } else {
                for mut chunk in line_rope.chunks() {
                    let chunk_width = chunk.display_width();
                    if chunk_width <= width_remaining {
                        spans.push(Span::Text {
                            char_range: text_x..(text_x + chunk_width),
                        });
                        text_x += chunk_width;
                        width_remaining -= chunk_width;
                    } else {
                        // Needs a soft wrap.
                        let mut i = 0;
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

                            dbg!(&chunk[..wrap_byte_at]);
                            spans.push(Span::Text {
                                char_range: text_x..(text_x + wrap_byte_at),
                            });

                            text_x += wrap_char_at;
                            self.lines.push(DisplayLine {
                                spans,
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
                    dbg!(&line_rope.as_str().unwrap()[front.x..text_x], &text_x);
                    self.lines.push(DisplayLine {
                        spans,
                        range: Range::from_points(front, Point::new(text_y, text_x)),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn layout_without_softwrap() {
        let mut view = View::new();
        let buffer = Buffer::from_str("123\nabc\n\nxyz");
        view.layout(&buffer, 5, 3);
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
        view.layout(&buffer, 5, 3);
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
        view.layout(&buffer, 5, 3);
        assert_eq!(view.lines.len(), 3);
        assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 5));
        assert_eq!(view.lines[1].range, Range::new(0, 5, 0, 8));
        assert_eq!(view.lines[2].range, Range::new(1, 0, 1, 3));
    }

    #[test]
    fn layout_with_softwrap2() {
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#$%\nxyz");
        view.layout(&buffer, 5, 3);
        assert_eq!(view.lines.len(), 4);
        assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 5));
        assert_eq!(view.lines[1].range, Range::new(0, 5, 0, 10));
        assert_eq!(view.lines[2].range, Range::new(0, 10, 0, 15));
        assert_eq!(view.lines[3].range, Range::new(1, 0, 1, 3));
    }

    #[test]
    fn point_to_display_line() {
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#$%\nxyz");
        view.layout(&buffer, 5, 3);
        assert_eq!(view.point_to_display_line(&Point::new(0, 0)), Some(0));
        assert_eq!(view.point_to_display_line(&Point::new(0, 5)), Some(1));
        assert_eq!(view.point_to_display_line(&Point::new(0, 14)), Some(2));
        assert_eq!(view.point_to_display_line(&Point::new(1, 2)), Some(3));
        assert_eq!(view.point_to_display_line(&Point::new(0, 16)), None);
        assert_eq!(view.point_to_display_line(&Point::new(1, 3)), None);
    }

    #[test]
    fn move_cursor_horizontally() {
        // 12345
        // abcde
        // !@#
        // xyz
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#$\nxyz");
        view.layout(&buffer, 5, 3);
        assert_eq!(
            // 1|2345
            view.compute_new_cursor_pos(&Point::new(0, 1), 1, 0),
            // a|bcde
            Point::new(0, 6)
        );
        assert_eq!(
            // a|bcde
            view.compute_new_cursor_pos(&Point::new(0, 6), 1, 0),
            // !|@#
            Point::new(0, 11)
        );
        assert_eq!(
            // !|@#
            view.compute_new_cursor_pos(&Point::new(0, 11), 1, 0),
            // x|yz
            Point::new(1, 1)
        );

        assert_eq!(
            // x|yz
            view.compute_new_cursor_pos(&Point::new(1, 1), -1, 0),
            // !|@#
            Point::new(0, 11)
        );
        assert_eq!(
            // !|@#
            view.compute_new_cursor_pos(&Point::new(0, 11), -1, 0),
            // a|bcde
            Point::new(0, 6)
        );
        assert_eq!(
            // !|@#
            view.compute_new_cursor_pos(&Point::new(0, 6), -1, 0),
            // a|bcde
            Point::new(0, 1)
        );
    }

    #[test]
    fn move_cursor_vertically() {
        // let mut view = View::new();
        // let buffer = Buffer::from_str("12345abcde!@#$%\nxyz");
        // view.layout(&buffer, 5, 3);
        // assert_eq!(view.lines.len(), 4);
        // assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 5));
        // assert_eq!(view.lines[1].range, Range::new(0, 5, 0, 10));
        // assert_eq!(view.lines[2].range, Range::new(0, 10, 0, 15));
        // assert_eq!(view.lines[3].range, Range::new(1, 0, 1, 3));
    }
}
