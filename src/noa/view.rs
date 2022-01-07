use core::num;
use std::cmp::min;

use arrayvec::ArrayString;
use noa_buffer::{
    buffer::Buffer,
    cursor::{Position, Range},
    display_width::DisplayWidth,
};

use crate::{
    highlighting::Highlighter,
    ui::canvas::{Grapheme, Style},
};

#[derive(Debug, PartialEq)]
pub struct Span {
    pub range: Range,
    pub style: Style,
}

#[derive(Debug, PartialEq)]
pub struct DisplayRow {
    /// The graphemes in this row.
    pub graphemes: Vec<Grapheme>,
    /// The positions in the buffer for each grapheme.
    pub positions: Vec<Position>,
}

pub struct View {
    rows: Vec<DisplayRow>,
    logical_x: usize,
    highlighter: Highlighter,
}

impl View {
    pub fn new(highlighter: Highlighter) -> View {
        View {
            rows: Vec::new(),
            logical_x: 0,
            highlighter,
        }
    }

    pub fn rows(&self) -> &[DisplayRow] {
        &self.rows
    }

    /// Called when the buffer is modified.
    pub fn update(&mut self, buffer: &Buffer) {
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

        // Locate the first grapheme's position in the given display rows.
        let first_pos = 'outer1: loop {
            for i in rows.clone() {
                match self.rows[i].positions.first() {
                    Some(pos) => break 'outer1 *pos,
                    None => continue,
                }
            }

            // No graphemes in the given rows.
            return;
        };

        // Locate the last grapheme's position in the given display rows.
        let last_pos = 'outer2: loop {
            for i in rows.clone().rev() {
                match self.rows[i].positions.last() {
                    Some(pos) => break 'outer2 *pos,
                    None => continue,
                }
            }

            // No graphemes in the given rows.
            return;
        };

        // Skip out-of-bounds spans.
        let mut spans_iter = spans.iter();
        while let Some(span) = spans_iter.next() {
            if span.range.back() > first_pos {
                spans_iter.next_back();
                break;
            }
        }

        // Apply spans.
        let mut row_i = rows.start;
        let mut col_i = 0;
        'outer: for span in spans {
            if span.range.front() > last_pos {
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

                grapheme.style = span.style;
                col_i += 1;
            }
        }
    }

    /// Computes the grapheme layout (text wrapping).
    ///
    /// If you want to disable soft wrapping. Set `width` to `std::max::MAX`.
    pub fn layout(&mut self, buffer: &Buffer, rows: usize, width: usize) {
        if rows == 1 {
            // The number of rows to be layouted is only 1, the naive approach
            // performs slightly better (around 500 nanoseconds). It's small
            // enough to use the parallelized approach for all cases though.
            self.layout_naive(buffer, rows, width);
        } else {
            self.layout_parallelized(buffer, rows, width);
        }
    }

    /// A naive implementation of the layout.
    fn layout_naive(&mut self, buffer: &Buffer, rows: usize, width: usize) {
        self.rows.clear();
        let mut y = 0;
        let num_lines = buffer.num_lines();
        while self.rows.len() < rows && y < num_lines {
            let rows = self.layout_line(buffer, y, width);
            debug_assert!(!rows.is_empty());
            self.rows.extend(rows);
            y += 1;
        }
    }

    /// A parallelized implementation of the layout.
    fn layout_parallelized(&mut self, buffer: &Buffer, rows: usize, width: usize) {
        use rayon::prelude::*;
        self.rows = (0..min(rows, buffer.num_lines()))
            .into_par_iter()
            .map(|y| {
                let rows = self.layout_line(buffer, y, width);
                debug_assert!(!rows.is_empty());
                rows
            })
            .flatten()
            .collect();
    }

    fn layout_line(&self, buffer: &Buffer, y: usize, width: usize) -> Vec<DisplayRow> {
        let range = Range::new(y, 0, y + 1, 0);
        let mut grapheme_iter = buffer.grapheme_iter(range);
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
                        Some(rope) => rope,
                        None => {
                            // Reached at EOF.
                            should_return = true;
                            break;
                        }
                    };

                // Turn the grapheme into a string `chars`.
                let mut chars = ArrayString::new();
                for ch in grapheme_rope.chars() {
                    chars.push(ch);
                }

                match chars.as_str() {
                    "\t" => {
                        // Compute the number of spaces to fill.
                        let mut n = 1;
                        while (pos.x + n) % buffer.config().tab_width != 0 && width_remaining > 0 {
                            dbg!(pos.x, n);
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
                graphemes,
                positions,
            });
        }

        rows
    }
}

// tree_sitter_mapping: phf_map! {
//     "comment" => SyntaxSpanType::Comment,
//     "identifier" => SyntaxSpanType::Ident,
//     "string_literal" => SyntaxSpanType::StringLiteral,
//     "primitive_type" => SyntaxSpanType::PrimitiveType,
//     "escape_sequence" => SyntaxSpanType::EscapeSequence,
//     "preproc_include" => SyntaxSpanType::CMacro,
//     "#include" => SyntaxSpanType::CMacro,
//     "system_lib_string" => SyntaxSpanType::CIncludeArg,
// },

#[cfg(test)]
mod tests {
    use crossterm::style::Color;
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

    #[test]
    fn test_highlight() {
        use Color::Red;

        let mut view = View::new(Highlighter::new(&PLAIN));

        let buffer = Buffer::from_text("ABC");
        view.layout(&buffer, 1, 3);
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
                graphemes: vec![g2("A", Red), g2("B", Red), g("C")],
                positions: vec![p(0, 0), p(0, 1), p(0, 2)],
            },]
        );
    }

    #[test]
    fn test_layout() {
        let mut view = View::new(Highlighter::new(&PLAIN));

        let buffer = Buffer::from_text("");
        view.layout(&buffer, 3, 5);
        assert_eq!(view.rows().len(), 1);
        assert_eq!(view.rows()[0].graphemes, vec![]);
        assert_eq!(view.rows()[0].positions, vec![]);

        let buffer = Buffer::from_text("ABC\nX\nY");
        view.layout(&buffer, 3, 5);
        assert_eq!(view.rows().len(), 3);
        assert_eq!(view.rows()[0].graphemes, vec![g("A"), g("B"), g("C")]);
        assert_eq!(view.rows()[0].positions, vec![p(0, 0), p(0, 1), p(0, 2)]);
        assert_eq!(view.rows()[1].graphemes, vec![g("X")]);
        assert_eq!(view.rows()[1].positions, vec![p(1, 0)]);
        assert_eq!(view.rows()[2].graphemes, vec![g("Y")]);
        assert_eq!(view.rows()[2].positions, vec![p(2, 0)]);

        // Soft wrapping.
        let buffer = Buffer::from_text("ABC123XYZ");
        view.layout(&buffer, 2 /* at least 2 */, 3);
        assert_eq!(view.rows().len(), 3);
        assert_eq!(view.rows()[0].graphemes, vec![g("A"), g("B"), g("C")]);
        assert_eq!(view.rows()[0].positions, vec![p(0, 0), p(0, 1), p(0, 2)]);
        assert_eq!(view.rows()[1].graphemes, vec![g("1"), g("2"), g("3")]);
        assert_eq!(view.rows()[1].positions, vec![p(0, 3), p(0, 4), p(0, 5)]);
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
        view.layout(&buffer, 1, 16);
        assert_eq!(view.rows().len(), 1);
        assert_eq!(
            view.rows()[0].graphemes,
            vec![g(" "), g(" "), g(" "), g(" "), g("A")]
        );
        assert_eq!(
            view.rows()[0].positions,
            vec![p(0, 0), p(0, 0), p(0, 0), p(0, 0), p(0, 1)]
        );

        let mut buffer = Buffer::from_text("AB\tC");
        buffer.set_config(&config);
        view.layout(&buffer, 1, 16);
        assert_eq!(view.rows().len(), 1);
        assert_eq!(
            view.rows()[0].graphemes,
            vec![g("A"), g("B"), g(" "), g(" "), g("C")]
        );
        assert_eq!(
            view.rows()[0].positions,
            vec![p(0, 0), p(0, 1), p(0, 2), p(0, 2), p(0, 3)]
        );

        let mut buffer = Buffer::from_text("ABC\t\t");
        buffer.set_config(&config);
        view.layout(&buffer, 1, 16);
        assert_eq!(view.rows().len(), 1);
        assert_eq!(
            view.rows()[0].graphemes,
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
            view.rows()[0].positions,
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

    #[bench]
    fn bench_layout_single_line(b: &mut test::Bencher) {
        let mut view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(1));
        b.iter(|| view.layout(&buffer, 1, 120));
    }

    #[bench]
    fn bench_layout_medium_text(b: &mut test::Bencher) {
        let mut view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(2048));
        b.iter(|| view.layout(&buffer, 2048, 120));
    }

    #[bench]
    fn bench_layout_single_line_naive(b: &mut test::Bencher) {
        let mut view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(1));
        b.iter(|| view.layout_naive(&buffer, 1, 120));
    }

    #[bench]
    fn bench_layout_tiny_text_naive(b: &mut test::Bencher) {
        let mut view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(8));
        b.iter(|| view.layout_naive(&buffer, 8, 120));
    }

    #[bench]
    fn bench_layout_single_line_parallelized(b: &mut test::Bencher) {
        let mut view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(1));
        b.iter(|| view.layout_parallelized(&buffer, 1, 120));
    }

    #[bench]
    fn bench_layout_tiny_text_parallelized(b: &mut test::Bencher) {
        let mut view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(8));
        b.iter(|| view.layout_parallelized(&buffer, 8, 120));
    }
}
