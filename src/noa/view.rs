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

    /// Computes the grapheme layout.
    ///
    /// If you want to disable soft wrapping. Set `width` to `std::max::MAX`.
    pub fn layout(&mut self, buffer: &Buffer, rows: usize, width: usize) {
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
                        todo!()
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
    use noa_languages::definitions::PLAIN;

    use super::*;

    fn g(c: &str) -> Grapheme {
        Grapheme {
            chars: ArrayString::from(c).unwrap(),
            style: Style::default(),
        }
    }

    fn p(y: usize, x: usize) -> Position {
        Position::new(y, x)
    }

    #[test]
    fn test_layout() {
        let mut view = View::new(Highlighter::new(&PLAIN));

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

    #[bench]
    fn bench_layout_tiny_text(b: &mut test::Bencher) {
        let mut view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(8));
        b.iter(|| view.layout(&buffer, 8, 120));
    }

    #[bench]
    fn bench_layout_small_text(b: &mut test::Bencher) {
        let mut view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(256));
        b.iter(|| view.layout(&buffer, 256, 120));
    }

    #[bench]
    fn bench_layout_medium_text(b: &mut test::Bencher) {
        let mut view = View::new(Highlighter::new(&PLAIN));
        let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(4096));
        b.iter(|| view.layout(&buffer, 4096, 120));
    }

    // #[bench]
    // fn bench_layout_large_text(b: &mut test::Bencher) {
    //     let mut view = View::new(Highlighter::new(&PLAIN));
    //     let buffer = Buffer::from_text(&(format!("{}\n", "A".repeat(80))).repeat(50000));
    //     b.iter(|| view.layout(&buffer, 50000, 120));
    // }
}
