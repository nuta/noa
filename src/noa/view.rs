//! Text reflow (soft wrapping), cursor movement, and scroll.

use std::cmp::{self, max, min};

use std::ops;

use noa_buffer::{Buffer, Cursor, Point, Range};
use noa_langs::{tree_sitter, HighlightType, Lang};

use noa_cui::DisplayWidth;

#[derive(Debug, Clone, PartialEq)]
pub struct Highlight {
    /// Range in the display line.
    pub range: ops::Range<usize>,
    pub highlight_type: HighlightType,
}

#[derive(Debug, Clone)]
pub struct DisplayLine {
    pub buffer_y: usize,
    /// The char indices in a line rope.
    pub chunks: Vec<ops::Range<usize>>,
    /// The char indices in the whole buffer rope.
    pub range: Range,
    pub syntax_highlights: Vec<Highlight>,
    pub search_highlights: Vec<Highlight>,
}

pub struct View {
    lines: Vec<DisplayLine>,
    /// The index of display line.
    scroll: usize,
    height: usize,
    logical_x: usize,
}

impl View {
    pub fn new() -> View {
        View {
            lines: Vec::new(),
            scroll: 0,
            height: 0,
            logical_x: 0,
        }
    }

    /// Returns `(screen_y, screen_x)`.
    pub fn point_to_display_pos(
        &self,
        pos: Point,
        screen_y_end: usize,
        screen_text_start: usize,
        buffer_num_lines: usize,
    ) -> (usize, usize) {
        self.point_to_display_line(pos)
            .map(|i| {
                let display_line = &self.lines[i];
                (
                    i - self.scroll,
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

    pub fn display_pos_to_point(&self, display_y: usize, display_x: usize) -> Option<Point> {
        self.lines.get(self.scroll + display_y).map(|line| {
            let mut pos = line.range.front();
            pos.x += min(display_x, line.range.end.x);
            pos
        })
    }

    fn point_to_display_line(&self, pos: Point) -> Option<usize> {
        if matches!(self.lines.last(), Some(line) if line.range.back().y + 1 == pos.y) && pos.x == 0
        {
            // EOF.
            return Some(self.lines.len() - 1);
        }

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
        &self.lines[self.scroll..min(self.lines.len(), self.scroll + self.height)]
    }

    pub fn layout(&mut self, buffer: &Buffer, y_from: usize, height: usize, width: usize) {
        if y_from == 0 {
            self.lines.clear();
        } else {
            self.lines
                .truncate(self.point_to_display_line(Point::new(y_from, 0)).unwrap());
        }

        for text_y in y_from..buffer.num_lines() {
            let line_rope = buffer.line(text_y);
            let mut spans = Vec::new();
            let mut width_remaining = width;
            let mut text_x = 0;
            let mut front = Point::new(text_y, text_x);

            if line_rope.len_chars() == 0 {
                self.lines.push(DisplayLine {
                    buffer_y: text_y,
                    chunks: vec![],
                    range: Range::from_points(Point::new(text_y, 0), Point::new(text_y, 0)),
                    syntax_highlights: vec![],
                    search_highlights: vec![],
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
                                buffer_y: text_y,
                                chunks: spans,
                                range: Range::from_points(front, Point::new(text_y, text_x)),
                                syntax_highlights: vec![],
                                search_highlights: vec![],
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
                        buffer_y: text_y,
                        chunks: spans,
                        range: Range::from_points(front, Point::new(text_y, text_x)),
                        syntax_highlights: vec![],
                        search_highlights: vec![],
                    });
                }
            }
        }

        self.height = height;

        let i = self
            .point_to_display_line(buffer.main_cursor_pos())
            .unwrap();

        // Scroll up.
        if i < self.scroll {
            self.scroll = i;
        }

        // Scroll down.
        if i >= self.scroll + height {
            self.scroll = i - height + 1;
        }
    }

    pub fn set_cursor(&mut self, buffer: &mut Buffer, cursor: Cursor) {
        buffer.set_cursors(vec![cursor]);
        self.logical_x = buffer.main_cursor_pos().x;
    }

    pub fn scroll(&mut self, buffer: &mut Buffer, y_diff: isize) {
        self.scroll = if y_diff < 0 {
            self.scroll.saturating_sub(y_diff.abs() as usize)
        } else {
            min(
                self.scroll + y_diff.abs() as usize,
                self.lines.len().saturating_sub(1),
            )
        };

        let dl = &self.lines[self.scroll];
        buffer.set_cursors(vec![Cursor::from(dl.range.start)]);
    }

    pub fn move_cursors(&mut self, buffer: &mut Buffer, y_diff: isize, x_diff: isize) {
        let mut new_cursors = Vec::new();

        for (i, cursor) in buffer.cursors().iter().enumerate() {
            // Cancel the selection.
            let old_pos = match cursor {
                Cursor::Normal { pos, .. } => *pos,
                Cursor::Selection(range) => range.end,
            };

            let new_pos = self.move_x(self.move_y(old_pos, y_diff, Some(self.logical_x)), x_diff);
            if i == 0 {
                // The main cursor.
                if y_diff.abs() == 0 {
                    self.logical_x = new_pos.x;
                }
            }

            new_cursors.push(Cursor::Normal { pos: new_pos });
        }

        buffer.set_cursors(new_cursors);
    }

    pub fn expand_selections(&self, buffer: &mut Buffer, y_diff: isize, x_diff: isize) {
        let mut new_cursors = Vec::new();
        for cursor in buffer.cursors() {
            let (start, end) = match cursor {
                Cursor::Normal { pos, .. } => (*pos, *pos),
                Cursor::Selection(range) => (range.start, range.end),
            };

            // Move the cursor.
            let new_end = self.move_x(self.move_y(end, y_diff, None), x_diff);
            new_cursors.push(Cursor::Selection(Range::from_points(start, new_end)));
        }

        buffer.set_cursors(new_cursors);
    }

    fn move_y(&self, pos: Point, y_diff: isize, logical_x: Option<usize>) -> Point {
        let prev_y = self.point_to_display_line(pos).unwrap();
        let prev_line = &self.lines[prev_y];

        let new_y = if y_diff < 0 {
            prev_y.saturating_sub(y_diff.abs() as usize)
        } else {
            prev_y + y_diff.abs() as usize
        };

        let new_line = &self
            .lines
            .get(new_y)
            .unwrap_or_else(|| &self.lines[self.lines.len() - 1]);

        let char_x = max(logical_x.unwrap_or(0), pos.x).saturating_sub(prev_line.range.front().x);

        Point::new(
            new_line.range.front().y,
            min(new_line.range.front().x + char_x, new_line.range.back().x),
        )
    }

    fn move_x(&self, pos: Point, x_diff: isize) -> Point {
        let current_y = self.point_to_display_line(pos).unwrap();
        let current_line = &self.lines[current_y];
        let mut new_pos = pos;

        if x_diff > 0 {
            assert!(x_diff == 1);
            let new_x = pos.x + 1;
            if new_x < current_line.range.back().x {
                new_pos.x = new_x;
            } else if let Some(next_line) = self.lines.get(current_y + 1) {
                new_pos = next_line.range.front();
            }
        } else if x_diff == -1 {
            if pos.x > 0 && pos.x > current_line.range.front().x {
                new_pos.x = pos.x - 1;
            } else if current_y > 0 {
                if let Some(prev_line) = self.lines.get(current_y - 1) {
                    new_pos = prev_line.range.back();
                }
            }
        }

        new_pos
    }

    fn walk_ts_node<'a, 'b, 'tree>(
        &'a mut self,
        lang: &'static Lang,
        parent: tree_sitter::Node<'tree>,
        cursor: &'b mut tree_sitter::TreeCursor<'tree>,
    ) {
        for node in parent.children(cursor) {
            let start_position = node.start_position();
            let end_position = node.end_position();
            if end_position.row == start_position.row {
                let start = Point::new(start_position.row, start_position.column);
                let end = Point::new(end_position.row, end_position.column);
                let range = Range::from_points(start, end);

                for line in &mut self.lines {
                    if range.overlaps_with(&line.range) {
                        if let Some(highlight_type) =
                            lang.tree_sitter_mapping.get(node.kind()).copied()
                        {
                            let start_pos = max(start, line.range.start);
                            let end_pos = min(end, line.range.end);
                            let range = (start_pos.x - line.range.start.x)
                                ..(end_pos.x - line.range.start.x);

                            line.syntax_highlights.push(Highlight {
                                range,
                                highlight_type,
                            });
                        }
                    }
                }
            }

            let mut node_cursor = node.walk();
            if node.child_count() > 0 {
                self.walk_ts_node(lang, node, &mut node_cursor);
            }
        }
    }

    pub fn highlight_from_tree_sitter<'tree>(
        &mut self,
        lang: &'static Lang,
        tree: &'tree tree_sitter::Tree,
    ) {
        let root = tree.root_node();
        self.walk_ts_node(lang, root, &mut root.walk());
    }

    pub fn set_search_highlights(&mut self, matches: &[Range]) {
        for line in &mut self.lines {
            line.search_highlights.clear();
        }

        for Range { start, end } in matches {
            for line in &mut self.lines {
                let range = Range::from_points(*start, *end);
                if range.contains_range(&line.range) {
                    trace!("search highlight: {}..{}", start, end);
                    let start_pos = max(start, &line.range.start);
                    let end_pos = min(end, &line.range.end);
                    let range =
                        (start_pos.x - line.range.start.x)..(end_pos.x - line.range.start.x);
                    line.search_highlights.push(Highlight {
                        range,
                        highlight_type: HighlightType::MatchedBySearch,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use noa_langs::tree_sitter::Tree;

    use super::*;

    #[test]
    fn layout_without_softwrap() {
        let mut view = View::new();
        let buffer = Buffer::from_str("123\nabc\n\nxyz");
        view.layout(&buffer, 0, 3, 5);
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
        view.layout(&buffer, 0, 3, 5);
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
        view.layout(&buffer, 0, 3, 5);
        assert_eq!(view.lines.len(), 3);
        assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 5));
        assert_eq!(view.lines[1].range, Range::new(0, 5, 0, 8));
        assert_eq!(view.lines[2].range, Range::new(1, 0, 1, 3));
    }

    #[test]
    fn layout_with_softwrap2() {
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#$%\nxyz\nLMNO");
        view.layout(&buffer, 0, 3, 5);
        assert_eq!(view.lines.len(), 5);
        assert_eq!(view.lines[0].range, Range::new(0, 0, 0, 5));
        assert_eq!(view.lines[1].range, Range::new(0, 5, 0, 10));
        assert_eq!(view.lines[2].range, Range::new(0, 10, 0, 15));
        assert_eq!(view.lines[3].range, Range::new(1, 0, 1, 3));
        assert_eq!(view.lines[4].range, Range::new(2, 0, 2, 4));

        view.layout(&buffer, 1, 3, 5);
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
        view.layout(&buffer, 0, 3, 5);
        assert_eq!(view.point_to_display_line(Point::new(0, 0)), Some(0));
        assert_eq!(view.point_to_display_line(Point::new(0, 5)), Some(1));
        assert_eq!(view.point_to_display_line(Point::new(0, 14)), Some(2));
        assert_eq!(view.point_to_display_line(Point::new(0, 15)), Some(2));
        assert_eq!(view.point_to_display_line(Point::new(1, 16)), None);
        assert_eq!(view.point_to_display_line(Point::new(1, 2)), Some(3));
        assert_eq!(view.point_to_display_line(Point::new(1, 3)), Some(3));
        assert_eq!(view.point_to_display_line(Point::new(1, 4)), None);
    }

    #[test]
    fn move_x() {
        // 12345
        // abcde
        // !@#
        // xyz
        let mut view = View::new();
        let buffer = Buffer::from_str("12345abcde!@#\nxyz");
        view.layout(&buffer, 0, 3, 5);
        assert_eq!(
            // 1|2345
            view.move_x(Point::new(0, 1), 1),
            // 12|345
            Point::new(0, 2)
        );
        assert_eq!(
            // 1234|5
            view.move_x(Point::new(0, 4), 1),
            // both 12345| and |abcde
            Point::new(0, 5)
        );
        assert_eq!(
            // both 12345| and |abcde
            view.move_x(Point::new(0, 5), 1),
            // a|bcde
            Point::new(0, 6)
        );
        assert_eq!(
            // !@#|
            view.move_x(Point::new(0, 13), 1),
            // |xyz
            Point::new(1, 0)
        );

        assert_eq!(
            // 12|345
            view.move_x(Point::new(0, 2), -1),
            // 1|2345
            Point::new(0, 1)
        );
        assert_eq!(
            // |xyz
            view.move_x(Point::new(1, 0), -1),
            // !@#|
            Point::new(0, 13)
        );

        assert_eq!(
            // |12345
            view.move_x(Point::new(0, 0), -1),
            // |12345
            Point::new(0, 0)
        );
        assert_eq!(
            // xyz|
            view.move_x(Point::new(1, 3), 1),
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
        view.layout(&buffer, 0, 3, 5);
        assert_eq!(
            // 1|2345
            view.move_y(Point::new(0, 1), 1, None),
            // a|bcde
            Point::new(0, 6)
        );
        assert_eq!(
            // a|bcde
            view.move_y(Point::new(0, 6), 1, None),
            // !|@#
            Point::new(0, 11)
        );
        assert_eq!(
            // !|@#
            view.move_y(Point::new(0, 11), 1, None),
            // x|yz
            Point::new(1, 1)
        );

        assert_eq!(
            // x|yz
            view.move_y(Point::new(1, 1), -1, None),
            // !|@#
            Point::new(0, 11)
        );
        assert_eq!(
            // !|@#
            view.move_y(Point::new(0, 11), -1, None),
            // a|bcde
            Point::new(0, 6)
        );
        assert_eq!(
            // !|@#
            view.move_y(Point::new(0, 6), -1, None),
            // a|bcde
            Point::new(0, 1)
        );

        assert_eq!(
            // 1|2345
            view.move_y(Point::new(0, 1), -1, None),
            // 1|2345
            Point::new(0, 1)
        );
        assert_eq!(
            // x|yz
            view.move_y(Point::new(1, 1), 1, None),
            // x|yz
            Point::new(1, 1)
        );
    }

    #[test]
    fn logical_x() {
        // AB|C
        //
        // DEFGH
        // 1
        // XYZ
        let mut buffer = Buffer::from_str("ABC\n\nDEFGH\n1\nXYZ");
        let mut view = View::new();
        view.set_cursor(&mut buffer, Cursor::new(0, 2));
        view.layout(&buffer, 0, 10, 5);

        // DE|FGH
        view.move_cursors(&mut buffer, 2, 0);
        assert_eq!(buffer.cursors(), vec![Cursor::new(2, 2)]);
        // 1|
        view.move_cursors(&mut buffer, 1, 0);
        assert_eq!(buffer.cursors(), vec![Cursor::new(3, 1)]);
        // XY|Z
        view.move_cursors(&mut buffer, 1, 0);
        assert_eq!(buffer.cursors(), vec![Cursor::new(4, 2)]);
        // No changes.
        view.move_cursors(&mut buffer, 1, 0);
        assert_eq!(buffer.cursors(), vec![Cursor::new(4, 2)]);

        // 1|
        view.move_cursors(&mut buffer, -1, 0);
        assert_eq!(buffer.cursors(), vec![Cursor::new(3, 1)]);
        // DE|FGH
        view.move_cursors(&mut buffer, -1, 0);
        assert_eq!(buffer.cursors(), vec![Cursor::new(2, 2)]);
        // | (lineno 2)
        view.move_cursors(&mut buffer, -1, 0);
        assert_eq!(buffer.cursors(), vec![Cursor::new(1, 0)]);
        // AB|C
        view.move_cursors(&mut buffer, -1, 0);
        assert_eq!(buffer.cursors(), vec![Cursor::new(0, 2)]);
        // No changes.
        view.move_cursors(&mut buffer, -1, 0);
        assert_eq!(buffer.cursors(), vec![Cursor::new(0, 2)]);
    }

    fn create_and_highlight_buffer(lang: &'static Lang, text: &str) -> (Buffer, Tree) {
        let mut buffer = Buffer::from_str(text);

        buffer.set_lang(lang);
        let mut parser = buffer.lang().syntax_highlighting_parser().unwrap();
        let tree = parser.parse(buffer.text(), None).unwrap();
        (buffer, tree)
    }

    #[test]
    fn highlight_single_line() {
        // #include <stdio.h>
        let (buffer, tree) =
            create_and_highlight_buffer(&noa_langs::C, concat!("#include <stdio.h>\n",));
        let mut view = View::new();

        view.layout(&buffer, 0, 25, 80);
        view.highlight_from_tree_sitter(&noa_langs::C, &tree);

        assert_eq!(view.lines.len(), 2);
        assert_eq!(
            &view.lines[0].syntax_highlights,
            &[
                Highlight {
                    highlight_type: HighlightType::CMacro,
                    range: 0..8,
                },
                Highlight {
                    highlight_type: HighlightType::CIncludeArg,
                    range: 9..18,
                },
            ]
        );
    }

    #[test]
    fn highlight_multi_lined_node() {
        // "abcd
        // 12345
        // xyz"
        let (buffer, tree) =
            create_and_highlight_buffer(&noa_langs::C, concat!("\"abcd12345xyz\"\n",));
        let mut view = View::new();

        view.layout(&buffer, 0, 25, 5);
        view.highlight_from_tree_sitter(&noa_langs::C, &tree);

        assert_eq!(view.lines.len(), 4);
        assert_eq!(
            &view.lines[0].syntax_highlights,
            &[Highlight {
                highlight_type: HighlightType::StringLiteral,
                range: 0..5,
            }]
        );
        assert_eq!(
            &view.lines[1].syntax_highlights,
            &[Highlight {
                highlight_type: HighlightType::StringLiteral,
                range: 0..5,
            }]
        );
        assert_eq!(
            &view.lines[2].syntax_highlights,
            &[Highlight {
                highlight_type: HighlightType::StringLiteral,
                range: 0..4,
            }]
        );
    }

    #[test]
    fn highlight() {
        //  // This is C code.
        //  #include <stdio.h>
        //  int foo;
        let (buffer, tree) = create_and_highlight_buffer(
            &noa_langs::C,
            concat!("// This is C code.\n", "#include <stdio.h>\n", "int foo;\n",),
        );
        let mut view = View::new();

        view.layout(&buffer, 0, 25, 80);
        view.highlight_from_tree_sitter(&noa_langs::C, &tree);

        assert_eq!(view.lines.len(), 4);
        assert_eq!(
            &view.lines[0].syntax_highlights,
            &[Highlight {
                highlight_type: HighlightType::Comment,
                range: 0..18,
            },]
        );
        assert_eq!(
            &view.lines[1].syntax_highlights,
            &[
                Highlight {
                    highlight_type: HighlightType::CMacro,
                    range: 0..8,
                },
                Highlight {
                    highlight_type: HighlightType::CIncludeArg,
                    range: 9..18,
                },
            ]
        );
        assert_eq!(
            &view.lines[2].syntax_highlights,
            &[
                Highlight {
                    highlight_type: HighlightType::PrimitiveType,
                    range: 0..3,
                },
                Highlight {
                    highlight_type: HighlightType::Ident,
                    range: 4..7,
                },
            ]
        );
    }
}
