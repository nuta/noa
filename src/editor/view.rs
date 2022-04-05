use std::cmp::min;

use arrayvec::ArrayString;
use noa_buffer::{
    buffer::Buffer,
    cursor::{Position, Range},
    display_width::DisplayWidth,
};
use noa_common::debug_warn_once;
use noa_compositor::canvas::{Grapheme, Style};

use crate::config::theme_for;

#[derive(Debug, PartialEq)]
pub struct Span {
    pub range: Range,
    pub style: Style,
}

#[derive(Debug, PartialEq)]
pub struct DisplayRow {
    /// The line number. Starts at 1.
    pub lineno: usize,
    /// The number of characters (not graphemes) in the row.
    pub len_chars: usize,
    /// The graphemes in this row.
    pub graphemes: Vec<Grapheme>,
    /// The positions in the buffer for each grapheme.
    pub positions: Vec<Position>,
}

impl DisplayRow {
    pub fn len_chars(&self) -> usize {
        self.len_chars
    }

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
            .map(|pos| Position::new(pos.y, pos.x + 1))
            .unwrap_or_else(|| Position::new(self.lineno - 1, 0))
    }

    pub fn range(&self) -> Range {
        Range::from_positions(self.first_position(), self.last_position())
    }

    pub fn total_width(&self) -> usize {
        self.graphemes.iter().map(|g| g.width).sum()
    }

    pub fn locate_column_by_position(&self, pos: Position) -> Option<usize> {
        if let Ok(pos) = self.positions.binary_search(&pos) {
            return Some(pos);
        }

        if self.positions.is_empty() {
            return Some(0);
        }

        let last_pos = self.last_position();
        if last_pos.y == pos.y && last_pos.x + 1 == pos.x {
            return Some(pos.x - self.first_position().x);
        }

        None
    }
}

pub struct View {
    rows: Vec<DisplayRow>,
    scroll_x: usize,
    scroll_y: usize,
    height: usize,
    softwrap: bool,
}

impl View {
    pub fn new() -> View {
        View {
            rows: Vec::new(),
            scroll_x: 0,
            scroll_y: 0,
            height: 0,
            softwrap: true,
        }
    }

    pub fn visible_rows(&self) -> &[DisplayRow] {
        &self.rows[self.scroll_y..min(self.rows.len(), self.scroll_y + self.height)]
    }

    pub fn scroll_x(&self) -> usize {
        if self.softwrap {
            0
        } else {
            self.scroll_x
        }
    }

    pub fn all_rows(&self) -> &[DisplayRow] {
        &self.rows
    }

    pub fn first_visible_position(&self) -> Position {
        self.visible_rows()
            .first()
            .map(|row| row.first_position())
            .unwrap_or_else(|| Position::new(0, 0))
    }

    pub fn last_visible_position(&self) -> Position {
        if self.rows.is_empty() {
            return Position::new(0, 0);
        }

        let last_visible_row_index =
            min(self.rows.len(), self.scroll_y + self.height).saturating_sub(1);
        let row = &self.rows[last_visible_row_index];
        match self.rows.get(last_visible_row_index + 1) {
            Some(next_row) if next_row.lineno == row.lineno => row.last_position(),
            None | Some(_) => {
                // If the cursor is at EOF or at the end of a line (with no
                // following wrapped virtual lines), then the last visible should
                // be 1 character past from the last position the row.
                row.end_of_row_position()
            }
        }
    }

    pub fn visible_range(&self) -> Range {
        Range::from_positions(self.first_visible_position(), self.last_visible_position())
    }

    pub fn toggle_soft_wrap(&mut self) {
        self.softwrap = !self.softwrap;
        if !self.softwrap {
            self.scroll_x = 0;
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_y = self.scroll_y.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_y = min(self.scroll_y + 1, self.rows.len().saturating_sub(1));
    }

    pub fn centering(&mut self, pos: Position, height: usize) {
        match self.locate_row_by_position(pos) {
            Some((row_index, _)) => {
                self.scroll_y = row_index.saturating_sub(height);
            }
            None => {
                warn!("out of bounds centering: {:?}", pos);
            }
        };
    }

    /// Clears highlights in the given rows.
    pub fn clear_highlights(&mut self, height: usize) {
        for i in self.scroll_y..min(self.scroll_y + height, self.rows.len()) {
            for grapheme in &mut self.rows[i].graphemes {
                grapheme.style = Style::default();
            }
        }
    }

    pub fn clear_highlight(&mut self, range: Range) {
        self.do_highlight(range, Style::default());
    }

    /// Update characters' styles in the given range.
    pub fn highlight(&mut self, range: Range, theme_key: &str) {
        let style = theme_for(theme_key);
        if style == Style::default() {
            return;
        }

        self.do_highlight(range, style);
    }

    /// Update characters' styles in the given range.
    pub fn do_highlight(&mut self, range: Range, style: Style) {
        // We don't handle out of bounds ranges because if a buffer is rendered
        // before the tree-sitter finishes parsing, tree-sitter may report a
        // highlight ranges of the previous version, which may be out of
        // bounds.
        let (start_y, start_x) = match self.locate_row_by_position(range.front()) {
            Some(yx) => yx,
            None => {
                debug_warn_once!("out of bounds highlight: {:?}", range);
                return;
            }
        };
        let (end_y, end_x) = match self.locate_row_by_position(range.back()) {
            Some(yx) => yx,
            None => {
                debug_warn_once!("out of bounds highlight: {:?}", range);
                return;
            }
        };

        for y in start_y..=end_y {
            let row = &mut self.rows[y];
            let x_max = row.len_chars();
            let mut xs = if y == start_y && y == end_y && start_x == end_x {
                start_x..(end_x + 1)
            } else if y == start_y && y == end_y {
                start_x..end_x
            } else if y == start_y {
                start_x..x_max
            } else if y == end_y {
                0..end_x
            } else {
                0..x_max
            };

            xs.start = min(xs.start, x_max);
            xs.end = min(xs.end, x_max);
            for x in xs {
                row.graphemes[x].style = style;
            }
        }
    }

    /// Computes the grapheme layout (text wrapping).
    pub fn layout(&mut self, buffer: &Buffer, height: usize, width: usize) {
        use rayon::prelude::*;

        let layout_width = if self.softwrap {
            width
        } else {
            // Disable soft wrapping.
            std::usize::MAX
        };

        self.height = height;
        self.rows = (0..buffer.num_lines())
            .into_par_iter()
            .map(|y| {
                let rows = self.layout_line(buffer, y, layout_width);
                debug_assert!(!rows.is_empty());
                rows
            })
            .flatten()
            .collect();

        // Adjust scroll_y and scroll_x if necessary.
        let main_pos = buffer.main_cursor().moving_position();
        while main_pos < self.first_visible_position() {
            self.scroll_y -= 1;
        }

        while main_pos > self.last_visible_position() {
            self.scroll_y += 1;
        }

        if !self.softwrap {
            let col_index = self
                .locate_row_by_position(main_pos)
                .unwrap()
                .1
                .saturating_sub(1);

            // dbg!(main_pos, col_index, self.scroll_x, width);
            if col_index >= self.scroll_x + width {
                self.scroll_x = col_index - (width - 1);
            } else if col_index < self.scroll_x {
                self.scroll_x = col_index;
            }
        }

        debug_assert!(self.scroll_y < self.rows.len());
    }

    /// Layouts a single physical (separated by "\n") line.
    fn layout_line(&self, buffer: &Buffer, y: usize, width: usize) -> Vec<DisplayRow> {
        let mut grapheme_iter = buffer.grapheme_iter(Position::new(y, 0));
        let mut unprocessed_grapheme = None;
        let mut rows = Vec::with_capacity(2);
        let mut pos = Position::new(y, 0);
        let mut should_return = false;
        while !should_return {
            let mut graphemes = Vec::with_capacity(128);
            let mut positions = Vec::with_capacity(128);
            let mut len_chars = 0;
            let mut width_remaining = width;

            // Fill `graphemes`.
            //
            // If we have a grapheme next to the last character of the last row,
            // specifically `unprocessed_grapheme` is Some, avoid consuming
            // the grapheme iterator.
            loop {
                let (grapheme_pos, grapheme) =
                    match unprocessed_grapheme.take().or_else(|| grapheme_iter.next()) {
                        Some((pos, _)) if pos.y > y => {
                            should_return = true;
                            break;
                        }
                        None => {
                            should_return = true;
                            break;
                        }
                        Some((pos, grapheme)) => (pos, grapheme),
                    };

                // Turn the grapheme into a string `chars`.
                let mut chars = ArrayString::new();
                for ch in grapheme.chars() {
                    chars.push(ch);
                }

                match chars.as_str() {
                    "\t" => {
                        // Compute the number of spaces to fill.
                        let mut n = 1;
                        while (pos.x + n) % buffer.editorconfig().tab_width != 0
                            && width_remaining > 0
                        {
                            n += 1;
                            width_remaining -= 1;
                        }

                        for _ in 0..n {
                            graphemes.push(Grapheme {
                                chars: ArrayString::from(" ").unwrap(),
                                width: 1,
                                style: Style::default(),
                            });
                            positions.push(pos);
                            len_chars += 1;
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
                            unprocessed_grapheme = Some((grapheme_pos, grapheme));
                            break;
                        }

                        graphemes.push(Grapheme {
                            chars,
                            width: chars.display_width(),
                            style: Style::default(),
                        });
                        positions.push(pos);
                        len_chars += chars.chars().count();

                        width_remaining -= grapheme_width;
                        pos.x += 1;
                    }
                }
            }

            rows.push(DisplayRow {
                lineno: y + 1,
                len_chars,
                graphemes,
                positions,
            });
        }

        rows
    }

    /// Returns the index of the display row and the index within the row.
    pub fn locate_row_by_position(&self, pos: Position) -> Option<(usize, usize)> {
        let i_y = self
            .rows
            .partition_point(|row| pos >= row.first_position() || row.range().contains(pos));

        debug_assert!(i_y > 0);
        let row = &self.rows[i_y - 1];
        let i_x = row.locate_column_by_position(pos)?;
        Some((i_y - 1, i_x))
    }

    pub fn get_position_from_screen_yx(&self, y: usize, x: usize) -> Option<Position> {
        self.rows.get(self.scroll_y + y).map(|row| {
            row.positions
                .get(x)
                .copied()
                .unwrap_or_else(|| Position::new(row.lineno - 1, row.len_chars()))
        })
    }
}

#[cfg(test)]
mod tests {
    use noa_buffer::cursor::Cursor;
    use noa_compositor::canvas::{Color, Grapheme, Style};
    use noa_editorconfig::EditorConfig;
    use pretty_assertions::assert_eq;

    use super::*;

    fn g(c: &str) -> Grapheme {
        Grapheme {
            chars: ArrayString::from(c).unwrap(),
            width: c.display_width(),
            style: Style::default(),
        }
    }

    fn g2(c: &str, fg: Color) -> Grapheme {
        Grapheme {
            chars: ArrayString::from(c).unwrap(),
            width: c.display_width(),
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
        let view = View::new();
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(num_lines));
        (view, buffer)
    }

    #[test]
    fn test_highlight() {
        use Color::Red;

        let mut view = View::new();

        let buffer = Buffer::from_text("ABC");
        view.layout(&buffer, 1, 3);
        view.do_highlight(
            Range::new(0, 0, 0, 2),
            Style {
                fg: Red,
                ..Default::default()
            },
        );

        assert_eq!(
            view.rows,
            vec![DisplayRow {
                lineno: 1,
                len_chars: 3,
                graphemes: vec![g2("A", Red), g2("B", Red), g("C")],
                positions: vec![p(0, 0), p(0, 1), p(0, 2)],
            },]
        );
    }

    #[test]
    fn test_layout() {
        let mut view = View::new();

        let buffer = Buffer::from_text("");
        view.layout(&buffer, 1, 5);
        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.rows[0].graphemes, vec![]);
        assert_eq!(view.rows[0].positions, vec![]);

        let buffer = Buffer::from_text("ABC\nX\nY");
        view.layout(&buffer, 5, 5);
        assert_eq!(view.rows.len(), 3);
        assert_eq!(view.rows[0].graphemes, vec![g("A"), g("B"), g("C")]);
        assert_eq!(view.rows[0].positions, vec![p(0, 0), p(0, 1), p(0, 2)]);
        assert_eq!(view.rows[1].graphemes, vec![g("X")]);
        assert_eq!(view.rows[1].positions, vec![p(1, 0)]);
        assert_eq!(view.rows[2].graphemes, vec![g("Y")]);
        assert_eq!(view.rows[2].positions, vec![p(2, 0)]);

        // Soft wrapping.
        let buffer = Buffer::from_text("ABC123XYZ");
        view.layout(&buffer, 1, 3);
        assert_eq!(view.rows.len(), 3);
        assert_eq!(view.rows[0].graphemes, vec![g("A"), g("B"), g("C")]);
        assert_eq!(view.rows[0].positions, vec![p(0, 0), p(0, 1), p(0, 2)]);
        assert_eq!(view.rows[1].graphemes, vec![g("1"), g("2"), g("3")]);
        assert_eq!(view.rows[1].positions, vec![p(0, 3), p(0, 4), p(0, 5)]);
    }

    #[test]
    fn test_layout_tabs() {
        let mut view = View::new();

        let config = EditorConfig {
            tab_width: 4,
            ..Default::default()
        };

        let mut buffer = Buffer::from_text("\tA");
        buffer.set_editorconfig(config);
        view.layout(&buffer, 1, 16);
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
        buffer.set_editorconfig(config);
        view.layout(&buffer, 1, 16);
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
        buffer.set_editorconfig(config);
        view.layout(&buffer, 1, 16);
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
    fn test_horizontal_scrolling() {
        let mut view = View::new();
        view.toggle_soft_wrap();

        let mut buffer = Buffer::from_text("abcWXYZ");
        buffer.set_cursors_for_test(&[Cursor::new(0, 3)]);
        view.layout(&buffer, 0, 3);
        assert_eq!(view.scroll_x, 0);
        buffer.set_cursors_for_test(&[Cursor::new(0, 4)]);
        view.layout(&buffer, 0, 3);
        assert_eq!(view.scroll_x, 1);
        buffer.set_cursors_for_test(&[Cursor::new(0, 5)]);
        view.layout(&buffer, 0, 3);
        assert_eq!(view.scroll_x, 2);

        buffer.set_cursors_for_test(&[Cursor::new(0, 3)]);
        view.layout(&buffer, 0, 3);
        assert_eq!(view.scroll_x, 2);
        buffer.set_cursors_for_test(&[Cursor::new(0, 2)]);
        view.layout(&buffer, 0, 3);
        assert_eq!(view.scroll_x, 1);
        buffer.set_cursors_for_test(&[Cursor::new(0, 1)]);
        view.layout(&buffer, 0, 3);
        assert_eq!(view.scroll_x, 0);
    }

    #[test]
    fn locate_row_by_position() {
        // ""
        let buffer = Buffer::from_text("");
        let mut view = View::new();
        view.layout(&buffer, 1, 5);

        assert_eq!(view.locate_row_by_position(p(0, 0)), Some((0, 0)));

        // ABC
        let buffer = Buffer::from_text("ABC");
        let mut view = View::new();
        view.layout(&buffer, 1, 5);

        assert_eq!(view.locate_row_by_position(p(0, 0)), Some((0, 0)));

        // ABC
        // 12
        // XYZ
        let buffer = Buffer::from_text("ABC\n12\nXYZ");
        let mut view = View::new();
        view.layout(&buffer, 3, 5);

        assert_eq!(view.locate_row_by_position(p(0, 0)), Some((0, 0)));
        assert_eq!(view.locate_row_by_position(p(0, 1)), Some((0, 1)));
        assert_eq!(view.locate_row_by_position(p(0, 3)), Some((0, 3)));
        assert_eq!(view.locate_row_by_position(p(0, 4)), None);
        assert_eq!(view.locate_row_by_position(p(1, 0)), Some((1, 0)));
        assert_eq!(view.locate_row_by_position(p(1, 1)), Some((1, 1)));
        assert_eq!(view.locate_row_by_position(p(1, 2)), Some((1, 2)));
        assert_eq!(view.locate_row_by_position(p(2, 0)), Some((2, 0)));
        assert_eq!(view.locate_row_by_position(p(2, 1)), Some((2, 1)));
        assert_eq!(view.locate_row_by_position(p(2, 3)), Some((2, 3)));
        assert_eq!(view.locate_row_by_position(p(3, 0)), None);
    }

    #[test]
    fn locate_row_by_position_softwrapped() {
        // ABC
        // XY
        //
        let buffer = Buffer::from_text("ABCXY\n");
        let mut view = View::new();
        view.layout(&buffer, 3, 3);

        assert_eq!(view.locate_row_by_position(p(0, 2)), Some((0, 2))); // |C
        assert_eq!(view.locate_row_by_position(p(0, 3)), Some((1, 0))); // |X
        assert_eq!(view.locate_row_by_position(p(0, 4)), Some((1, 1))); // |Y
        assert_eq!(view.locate_row_by_position(p(0, 5)), Some((1, 2))); // Y|
        assert_eq!(view.locate_row_by_position(p(1, 0)), Some((2, 0)));
    }

    #[bench]
    fn bench_layout_single_line(b: &mut test::Bencher) {
        let (mut view, buffer) = create_view_and_buffer(1);
        b.iter(|| view.layout(&buffer, 4096, 120));
    }

    #[bench]
    fn bench_layout_small_text(b: &mut test::Bencher) {
        let (mut view, buffer) = create_view_and_buffer(64);
        b.iter(|| view.layout(&buffer, 4096, 120));
    }

    #[bench]
    fn bench_layout_medium_text(b: &mut test::Bencher) {
        let (mut view, buffer) = create_view_and_buffer(2048);
        b.iter(|| view.layout(&buffer, 4096, 120));
    }
}
