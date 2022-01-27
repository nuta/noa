use core::num;
use std::{
    cmp::{max, min},
    collections::HashMap,
};

use arrayvec::ArrayString;
use noa_buffer::{
    buffer::Buffer,
    cursor::{Cursor, Position, Range},
    display_width::DisplayWidth,
};
use noa_compositor::canvas::{Grapheme, Style};

use crate::highlighting::Highlighter;

#[derive(Debug, PartialEq)]
pub struct Span {
    pub range: Range,
    pub style: Style,
}

#[derive(Debug, PartialEq)]
pub struct DisplayRow {
    // The line number. Starts at 1.
    pub lineno: usize,
    /// The graphemes in this row.
    pub graphemes: Vec<Grapheme>,
    /// The positions in the buffer for each grapheme.
    pub positions: Vec<Position>,
}

impl DisplayRow {
    pub fn is_empty(&self) -> bool {
        self.graphemes.is_empty()
    }

    pub fn first_position(&self) -> Position {
        self.positions
            .first()
            .copied()
            .unwrap_or_else(|| Position::new(self.lineno - 1, 0))
    }

    pub fn last_position(&self) -> Position {
        self.positions
            .last()
            .copied()
            .unwrap_or_else(|| Position::new(self.lineno - 1, 0))
    }

    pub fn end_of_row_position(&self) -> Position {
        self.positions
            .last()
            .copied()
            .map(|pos| Position::new(pos.y, pos.x + 1))
            .unwrap_or_else(|| Position::new(self.lineno - 1, 0))
    }

    pub fn range(&self) -> Range {
        Range::from_positions(self.first_position(), self.last_position())
    }

    pub fn locate_column_by_position(&self, pos: Position) -> usize {
        if let Ok(pos) = self.positions.binary_search(&pos) {
            return pos;
        }

        if self.positions.is_empty() {
            return 0;
        }

        let last_pos = self.last_position();
        if last_pos.y == pos.y && last_pos.x + 1 == pos.x {
            return pos.x;
        }

        unreachable!("position is out of bounds in the view: {:?}", pos);
    }
}

pub struct View {
    rows: Vec<DisplayRow>,
    // The first grapheme's position in `rows`.
    first_pos: Position,
    // The last grapheme's position in `rows`.
    last_pos: Position,
    scroll: usize,
    highlighter: Highlighter,
    visual_xs: HashMap<Cursor, usize>,
}

impl View {
    pub fn new(highlighter: Highlighter) -> View {
        View {
            rows: Vec::new(),
            scroll: 0,
            highlighter,
            first_pos: Position::new(0, 0),
            last_pos: Position::new(0, 0),
            visual_xs: HashMap::new(),
        }
    }

    pub fn display_rows(&self) -> impl Iterator<Item = &'_ DisplayRow> {
        self.rows.iter().skip(self.scroll)
    }

    /// Called when the buffer is modified.
    pub fn post_update(&mut self, buffer: &Buffer) {
        self.highlighter.update(buffer);
    }

    /// Clears highlights in the given rows.
    pub fn clear_highlights(&mut self, rows: std::ops::Range<usize>) {
        for i in rows.start..min(rows.end, self.rows.len()) {
            for grapheme in &mut self.rows[i].graphemes {
                grapheme.style = Style::default();
            }
        }
    }

    /// Apply highlights. Caller must ensure that:
    ///
    /// - `spans` are sorted by their `range`s.
    /// - Ranges in `spans` do not overlap.
    /// - `rows` are not out of bounds: [`View::layout`] must be called before.
    pub fn highlight(&mut self, rows: std::ops::Range<usize>, spans: &[Span]) {
        let rows = rows.start..min(rows.end, self.rows.len());

        // Skip out-of-bounds spans.
        let mut spans_iter = spans.iter();
        while let Some(span) = spans_iter.next() {
            if span.range.back() > self.first_pos {
                spans_iter.next_back();
                break;
            }
        }

        // Apply spans.
        let mut row_i = rows.start;
        let mut col_i = 0;
        for span in spans {
            if span.range.front() > self.last_pos {
                // Reached to the out-of-bounds span.
                break;
            }

            // Apply `span`.
            loop {
                if row_i >= self.rows.len() {
                    break;
                }

                if col_i >= self.rows[row_i].positions.len() {
                    row_i += 1;
                    col_i = 0;
                    continue;
                }

                let pos = self.rows[row_i].positions[col_i];
                let grapheme = &mut self.rows[row_i].graphemes[col_i];
                if !span.range.contains(pos) {
                    break;
                }

                grapheme.style.merge(span.style);
                col_i += 1;
            }
        }
    }

    /// Moves the cursor to left by one grapheme.
    pub fn move_cursors_left(&mut self, buffer: &mut Buffer) {
        self.move_cursors_with(buffer, |buffer, c| c.move_left(buffer));
        self.visual_xs.clear();
    }

    /// Moves the cursor to right by one grapheme.
    pub fn move_cursors_right(&mut self, buffer: &mut Buffer) {
        self.move_cursors_with(buffer, |buffer, c| c.move_right(buffer));
        self.visual_xs.clear();
    }

    pub fn move_cursors_vertically<F>(&mut self, buffer: &mut Buffer, y_diff: isize, f: F)
    where
        F: Fn(&mut Cursor, Position),
    {
        let mut visual_xs = self.visual_xs.clone();
        let mut new_visual_xs = HashMap::new();
        self.move_cursors_with(buffer, |buffer, c| {
            let (i_y, i_x) = self.locate_row_by_position(c.moving_position());
            let dest_row = self.rows.get(if y_diff > 0 {
                i_y.saturating_add(y_diff.abs() as usize)
            } else {
                i_y.saturating_sub(y_diff.abs() as usize)
            });

            if let Some(dest_row) = dest_row {
                let visual_x = visual_xs.get(c).copied();
                let new_pos = dest_row
                    .positions
                    .get(max(i_x, visual_x.unwrap_or(i_x)))
                    .copied()
                    .unwrap_or_else(|| dest_row.end_of_row_position());
                f(c, new_pos);
                new_visual_xs.insert(c.clone(), visual_x.unwrap_or(i_x));
            }
        });
        self.visual_xs = new_visual_xs;
    }

    pub fn move_cursors_horizontally<F>(&mut self, buffer: &mut Buffer, x_diff: isize, f: F)
    where
        F: Fn(&mut Cursor, Position),
    {
        let (left, right) = if x_diff > 0 {
            (0, x_diff as usize)
        } else {
            (x_diff as usize, 0)
        };

        self.move_cursors_with(buffer, |buffer, c| {
            let mut new_pos = c.moving_position();
            new_pos.move_by(buffer, 0, 0, left, right);
            f(c, new_pos);
        });
    }

    /// Moves the cursor to up by one display row (respecting soft wrapping).
    pub fn move_cursors_up(&mut self, buffer: &mut Buffer) {
        self.move_cursors_vertically(buffer, -1, |c, pos| c.move_to(pos));
    }

    /// Moves the cursor to down by one display row (respecting soft wrapping).
    pub fn move_cursors_down(&mut self, buffer: &mut Buffer) {
        self.move_cursors_vertically(buffer, 1, |c, pos| c.move_to(pos));
    }

    pub fn select_up(&mut self, buffer: &mut Buffer) {
        self.move_cursors_vertically(buffer, -1, |c, pos| {
            c.move_moving_position_to(pos);
        });
    }
    pub fn select_down(&mut self, buffer: &mut Buffer) {
        self.move_cursors_vertically(buffer, 1, |c, pos| {
            c.move_moving_position_to(pos);
        });
    }
    pub fn select_left(&mut self, buffer: &mut Buffer) {
        self.move_cursors_horizontally(buffer, -1, |c, pos| {
            c.move_moving_position_to(pos);
        });
    }
    pub fn select_right(&mut self, buffer: &mut Buffer) {
        self.move_cursors_horizontally(buffer, 1, |c, pos| {
            c.move_moving_position_to(pos);
        });
    }

    pub fn select_until_beginning_of_line(&mut self, buffer: &mut Buffer) {
        buffer.deselect_cursors();

        self.move_cursors_with(buffer, |buffer, c| {
            let pos = c.moving_position();
            let left = pos.x;

            let mut new_pos = c.moving_position();
            new_pos.move_by(buffer, 0, 0, left, 0);
            c.move_moving_position_to(new_pos);
        });
    }

    pub fn select_until_end_of_line(&mut self, buffer: &mut Buffer) {
        buffer.deselect_cursors();

        self.move_cursors_with(buffer, |buffer, c| {
            let pos = c.moving_position();
            let right = buffer.line_len(pos.y) - pos.x;

            let mut new_pos = c.moving_position();
            new_pos.move_by(buffer, 0, 0, 0, right);
            c.move_moving_position_to(new_pos);
        });
    }

    fn move_cursors_with<F>(&self, buffer: &mut Buffer, mut f: F)
    where
        F: FnMut(&Buffer, &mut Cursor),
    {
        let mut new_cursors = buffer.cursors().to_vec();
        for c in &mut new_cursors {
            f(buffer, c);
        }

        buffer.set_cursors(&new_cursors);
    }

    /// Computes the grapheme layout (text wrapping).
    ///
    /// If you want to disable soft wrapping. Set `width` to `std::max::MAX`.
    pub fn layout(&mut self, buffer: &Buffer, width: usize) {
        use rayon::prelude::*;

        self.rows = (0..buffer.num_lines())
            .into_par_iter()
            .map(|y| {
                let rows = self.layout_line(buffer, y, width);
                debug_assert!(!rows.is_empty());
                rows
            })
            .flatten()
            .collect();

        // Locate the first grapheme's position in the given display rows.
        self.first_pos = (|| {
            for i in 0..self.rows.len() {
                match self.rows[i].positions.first() {
                    Some(pos) => return *pos,
                    None => continue,
                }
            }

            // No graphemes in `self.rows`.
            Position::new(0, 0)
        })();

        // Locate the last grapheme's position in the given display rows.
        self.last_pos = (|| {
            for i in (0..self.rows.len()).rev() {
                match self.rows[i].positions.last() {
                    Some(pos) => return *pos,
                    None => continue,
                }
            }

            // No graphemes in `self.rows`.
            Position::new(0, 0)
        })();
    }

    /// Layouts a single physical (separated by "\n") line.
    fn layout_line(&self, buffer: &Buffer, y: usize, width: usize) -> Vec<DisplayRow> {
        let mut grapheme_iter = buffer.grapheme_iter(Position::new(y, 0));
        let mut unprocessed_grapheme = None;
        let mut rows = Vec::with_capacity(2);
        let mut pos = Position::new(y, 0);
        let mut should_return = false;
        while !should_return {
            let mut graphemes = Vec::with_capacity(width);
            let mut positions = Vec::with_capacity(width);
            let mut width_remaining = width;

            // Fill `graphemes`.
            //
            // If we have a grapheme next to the last character of the last row,
            // specifically `unprocessed_grapheme` is Some, avoid consuming
            // the grapheme iterator.
            loop {
                let grapheme_rope =
                    match unprocessed_grapheme.take().or_else(|| grapheme_iter.next()) {
                        Some(_) if grapheme_iter.position().y > y => {
                            should_return = true;
                            break;
                        }
                        None => {
                            should_return = true;
                            break;
                        }
                        Some(rope) => rope,
                    };

                // Turn the grapheme into a string `chars`.
                let mut chars = ArrayString::new();
                for mut ch in grapheme_rope.chars() {
                    chars.push(ch);
                }

                match chars.as_str() {
                    "\t" => {
                        // Compute the number of spaces to fill.
                        let mut n = 1;
                        while (pos.x + n) % buffer.config().tab_width != 0 && width_remaining > 0 {
                            n += 1;
                            width_remaining -= 1;
                        }

                        for _ in 0..n {
                            graphemes.push(Grapheme {
                                chars: ArrayString::from(" ").unwrap(),
                                style: Style::default(),
                            });
                            positions.push(pos);
                        }

                        pos.x += 1;
                    }
                    "\r" => {
                        // Ignore carriage returns. We'll handle newlines in the
                        // "\n" pattern below.
                    }
                    "\n" => {
                        should_return = true;
                        break;
                    }
                    _ => {
                        let grapheme_width = chars.as_str().display_width();
                        if grapheme_width > width_remaining {
                            // Save the current grapheme so that the it will be
                            // processed again in the next display row.
                            unprocessed_grapheme = Some(grapheme_rope);
                            break;
                        }

                        graphemes.push(Grapheme {
                            chars,
                            style: Style::default(),
                        });
                        positions.push(pos);

                        width_remaining -= grapheme_width;
                        pos.x += 1;
                    }
                }
            }

            rows.push(DisplayRow {
                lineno: y + 1,
                graphemes,
                positions,
            });
        }

        rows
    }

    /// Returns the index of the display row and the index within the row.
    fn locate_row_by_position(&self, pos: Position) -> (usize, usize) {
        let i_y = self
            .rows
            .partition_point(|row| pos >= row.first_position() || row.range().contains(pos));

        debug_assert!(i_y > 0);
        let row = &self.rows[i_y - 1];
        (i_y - 1, row.locate_column_by_position(pos))
    }
}

#[cfg(test)]
mod tests {
    use noa_compositor::canvas::{Color, Grapheme, Style};
    use noa_editorconfig::EditorConfig;
    use noa_languages::definitions::PLAIN;

    use super::*;

    fn g(c: &str) -> Grapheme {
        Grapheme {
            chars: ArrayString::from(c).unwrap(),
            style: Style::default(),
        }
    }

    fn g2(c: &str, fg: Color) -> Grapheme {
        Grapheme {
            chars: ArrayString::from(c).unwrap(),
            style: Style {
                fg,
                ..Style::default()
            },
        }
    }

    fn p(y: usize, x: usize) -> Position {
        Position::new(y, x)
    }

    fn create_view_and_buffer(num_lines: usize) -> (View, Buffer) {
        let view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(2048));
        (view, buffer)
    }

    #[test]
    fn test_highlight() {
        use Color::Red;

        let mut view = View::new(Highlighter::new(&PLAIN));

        let buffer = Buffer::from_text("ABC");
        view.layout(&buffer, 3);
        view.highlight(
            0..3,
            &[Span {
                range: Range::new(0, 0, 0, 2),
                style: Style {
                    fg: Red,
                    ..Default::default()
                },
            }],
        );

        assert_eq!(
            view.rows,
            vec![DisplayRow {
                lineno: 1,
                graphemes: vec![g2("A", Red), g2("B", Red), g("C")],
                positions: vec![p(0, 0), p(0, 1), p(0, 2)],
            },]
        );
    }

    #[test]
    fn test_layout() {
        let mut view = View::new(Highlighter::new(&PLAIN));

        let buffer = Buffer::from_text("");
        view.layout(&buffer, 5);
        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.rows[0].graphemes, vec![]);
        assert_eq!(view.rows[0].positions, vec![]);

        let buffer = Buffer::from_text("ABC\nX\nY");
        view.layout(&buffer, 5);
        assert_eq!(view.rows.len(), 3);
        assert_eq!(view.rows[0].graphemes, vec![g("A"), g("B"), g("C")]);
        assert_eq!(view.rows[0].positions, vec![p(0, 0), p(0, 1), p(0, 2)]);
        assert_eq!(view.rows[1].graphemes, vec![g("X")]);
        assert_eq!(view.rows[1].positions, vec![p(1, 0)]);
        assert_eq!(view.rows[2].graphemes, vec![g("Y")]);
        assert_eq!(view.rows[2].positions, vec![p(2, 0)]);

        // Soft wrapping.
        let buffer = Buffer::from_text("ABC123XYZ");
        view.layout(&buffer, 3);
        assert_eq!(view.rows.len(), 3);
        assert_eq!(view.rows[0].graphemes, vec![g("A"), g("B"), g("C")]);
        assert_eq!(view.rows[0].positions, vec![p(0, 0), p(0, 1), p(0, 2)]);
        assert_eq!(view.rows[1].graphemes, vec![g("1"), g("2"), g("3")]);
        assert_eq!(view.rows[1].positions, vec![p(0, 3), p(0, 4), p(0, 5)]);
    }

    #[test]
    fn test_layout_tabs() {
        let mut view = View::new(Highlighter::new(&PLAIN));

        let config = EditorConfig {
            tab_width: 4,
            ..Default::default()
        };

        let mut buffer = Buffer::from_text("\tA");
        buffer.set_config(&config);
        view.layout(&buffer, 16);
        assert_eq!(view.rows.len(), 1);
        assert_eq!(
            view.rows[0].graphemes,
            vec![g(" "), g(" "), g(" "), g(" "), g("A")]
        );
        assert_eq!(
            view.rows[0].positions,
            vec![p(0, 0), p(0, 0), p(0, 0), p(0, 0), p(0, 1)]
        );

        let mut buffer = Buffer::from_text("AB\tC");
        buffer.set_config(&config);
        view.layout(&buffer, 16);
        assert_eq!(view.rows.len(), 1);
        assert_eq!(
            view.rows[0].graphemes,
            vec![g("A"), g("B"), g(" "), g(" "), g("C")]
        );
        assert_eq!(
            view.rows[0].positions,
            vec![p(0, 0), p(0, 1), p(0, 2), p(0, 2), p(0, 3)]
        );

        let mut buffer = Buffer::from_text("ABC\t\t");
        buffer.set_config(&config);
        view.layout(&buffer, 16);
        assert_eq!(view.rows.len(), 1);
        assert_eq!(
            view.rows[0].graphemes,
            vec![
                g("A"),
                g("B"),
                g("C"),
                g(" "),
                g(" "),
                g(" "),
                g(" "),
                g(" ")
            ]
        );
        assert_eq!(
            view.rows[0].positions,
            vec![
                p(0, 0),
                p(0, 1),
                p(0, 2),
                p(0, 3),
                p(0, 4),
                p(0, 4),
                p(0, 4),
                p(0, 4)
            ]
        );
    }

    #[test]
    fn locate_row_by_position() {
        // ""
        let buffer = Buffer::from_text("");
        let mut view = View::new(Highlighter::new(&PLAIN));
        view.layout(&buffer, 5);

        assert_eq!(view.locate_row_by_position(p(0, 0)), (0, 0));

        // ABC
        let buffer = Buffer::from_text("ABC");
        let mut view = View::new(Highlighter::new(&PLAIN));
        view.layout(&buffer, 5);

        assert_eq!(view.locate_row_by_position(p(0, 0)), (0, 0));

        // ABC
        // 12
        // XYZ
        let buffer = Buffer::from_text("ABC\n12\nXYZ");
        let mut view = View::new(Highlighter::new(&PLAIN));
        view.layout(&buffer, 5);

        assert_eq!(view.locate_row_by_position(p(0, 0)), (0, 0));
        assert_eq!(view.locate_row_by_position(p(0, 1)), (0, 1));
        assert_eq!(view.locate_row_by_position(p(0, 3)), (0, 3));
        assert_eq!(view.locate_row_by_position(p(1, 0)), (1, 0));
        assert_eq!(view.locate_row_by_position(p(1, 1)), (1, 1));
        assert_eq!(view.locate_row_by_position(p(1, 2)), (1, 2));
        assert_eq!(view.locate_row_by_position(p(2, 0)), (2, 0));
        assert_eq!(view.locate_row_by_position(p(2, 1)), (2, 1));
        assert_eq!(view.locate_row_by_position(p(2, 3)), (2, 3));
    }

    #[test]
    fn cursor_movement() {
        // ABC
        // 12
        // XYZ
        let mut buffer = Buffer::from_text("ABC\n12\nXYZ");
        let mut view = View::new(Highlighter::new(&PLAIN));
        view.layout(&buffer, 5);

        buffer.set_cursors(&[Cursor::new(2, 1)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(1, 1)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 1)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 1)]);

        buffer.set_cursors(&[Cursor::new(0, 1)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(1, 1)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(2, 1)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(2, 1)]);
    }

    #[test]
    fn cursor_movement_at_end_of_line() {
        // ABC
        // ABC
        // ABC
        let mut buffer = Buffer::from_text("ABC\nABC\nABC");
        let mut view = View::new(Highlighter::new(&PLAIN));
        view.layout(&buffer, 5);

        buffer.set_cursors(&[Cursor::new(2, 3)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(1, 3)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 3)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 3)]);

        buffer.set_cursors(&[Cursor::new(0, 3)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(1, 3)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(2, 3)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(2, 3)]);
    }

    #[test]
    fn cursor_movement_through_empty_text() {
        let mut buffer = Buffer::from_text("");
        let mut view = View::new(Highlighter::new(&PLAIN));
        view.layout(&buffer, 5);

        buffer.set_cursors(&[Cursor::new(0, 0)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 0)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 0)]);
    }

    #[test]
    fn cursor_movement_through_empty_lines() {
        // ""
        // ""
        // ""
        let mut buffer = Buffer::from_text("\n\n");
        let mut view = View::new(Highlighter::new(&PLAIN));
        view.layout(&buffer, 5);

        buffer.set_cursors(&[Cursor::new(2, 0)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(1, 0)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 0)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 0)]);

        buffer.set_cursors(&[Cursor::new(0, 0)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(1, 0)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(2, 0)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(2, 0)]);
    }

    #[test]
    fn cursor_movement_preserving_visual_x() {
        // "ABCDEFG"
        // "123"
        // ""
        // "HIJKLMN"
        let mut buffer = Buffer::from_text("ABCDEFG\n123\n\nHIJKLMN");
        let mut view = View::new(Highlighter::new(&PLAIN));
        view.layout(&buffer, 10);

        buffer.set_cursors(&[Cursor::new(3, 5)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(2, 0)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(1, 3)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 5)]);
        view.move_cursors_up(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(0, 5)]);

        buffer.set_cursors(&[Cursor::new(0, 5)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(1, 3)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(2, 0)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(3, 5)]);
        view.move_cursors_down(&mut buffer);
        assert_eq!(buffer.cursors(), &[Cursor::new(3, 5)]);
    }

    #[bench]
    fn bench_layout_single_line(b: &mut test::Bencher) {
        let (mut view, buffer) = create_view_and_buffer(1);
        b.iter(|| view.layout(&buffer, 120));
    }

    #[bench]
    fn bench_layout_small_line(b: &mut test::Bencher) {
        let (mut view, buffer) = create_view_and_buffer(64);
        b.iter(|| view.layout(&buffer, 120));
    }

    #[bench]
    fn bench_layout_medium_text(b: &mut test::Bencher) {
        let (mut view, buffer) = create_view_and_buffer(2048);
        b.iter(|| view.layout(&buffer, 120));
    }

    #[bench]
    fn bench_highlight_few_spans(b: &mut test::Bencher) {
        let (mut view, buffer) = create_view_and_buffer(16);
        let mut spans = Vec::new();
        for i in 0..16 {
            spans.push(Span {
                range: Range::new(i, 0, i, 1),
                style: Style::default(),
            });
        }

        view.layout(&buffer, 120);
        b.iter(|| {
            view.clear_highlights(0..16);
            view.highlight(0..16, &spans);
        });
    }

    #[bench]
    fn bench_highlight_medium_spans(b: &mut test::Bencher) {
        let (mut view, buffer) = create_view_and_buffer(2048);
        let mut spans = Vec::new();
        for i in 0..4096 {
            spans.push(Span {
                range: Range::new(1024, 0, 1024, 1),
                style: Style::default(),
            });
        }

        view.layout(&buffer, 120);
        b.iter(|| {
            view.clear_highlights(1024..(1024 + 128));
            view.highlight(1024..(1024 + 128), &spans);
        });
    }
}
