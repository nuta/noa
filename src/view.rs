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

    pub fn compute_new_cursor_pos(
        &mut self,
        pos: &Point,
        y_diff: isize,
        x_diff: isize,
        adjust_top_left: bool,
        height: usize,
    ) -> Point {
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

            let char_x = pos.x - prev_line.range.back().x;

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

        if adjust_top_left {
            let index = self.point_to_display_line(pos).unwrap();
            if index < self.top_left {
                self.top_left = index;
            }

            if index >= self.top_left + height {
                self.top_left = index - height + 1;
            }
        }

        new_pos
    }

    fn point_to_display_line(&self, pos: &Point) -> Option<usize> {
        self.lines
            .binary_search_by(|line| {
                if line.range.contains(pos) {
                    cmp::Ordering::Equal
                } else if pos < line.range.front() {
                    cmp::Ordering::Less
                } else {
                    cmp::Ordering::Greater
                }
            })
            .ok()
    }

    pub fn layout(&mut self, buffer: &Buffer, y_from: usize, width: usize, height: usize) {
        self.lines.truncate(y_from);
        for text_y in y_from..buffer.num_lines() {
            let mut line_rope = buffer.line(text_y);
            let mut spans = Vec::new();
            let mut width_remaining = width;
            let mut text_x = 0;
            let mut front = Point::new(text_y, text_x);
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
                    while !chunk.is_empty() {
                        let mut wrap_at = 0;
                        for ch in chunk.chars() {
                            if ch.display_width() > width_remaining {
                                break;
                            }

                            wrap_at += 1;
                        }

                        spans.push(Span::Text {
                            char_range: text_x..(text_x + wrap_at),
                        });

                        self.lines.push(DisplayLine {
                            spans,
                            range: Range::from_points(front, Point::new(text_y, text_x)),
                        });

                        spans = Vec::new();
                        chunk = &chunk[text_x + wrap_at..];
                        text_x += wrap_at;
                        front = Point::new(text_y, text_x);
                        width_remaining = width;
                    }
                }
            }

            self.lines.push(DisplayLine {
                spans,
                range: Range::from_points(front, Point::new(text_y, text_x)),
            });
        }
    }
}
