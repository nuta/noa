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
    graphemes: Vec<Grapheme>,
    /// The positions in the buffer for each grapheme.
    positions: Vec<Position>,
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
    /// If you want to disable soft wrapping. Set `cols` to `std::max::MAX`.
    pub fn layout(&mut self, buffer: &Buffer, rows: usize, cols: usize) {
        self.rows.clear();
        let whole_buffer = Range::new(0, 0, buffer.num_lines(), 0);
        let mut pos = Position::new(0, 0);
        let mut grapheme_iter = buffer.grapheme_iter(whole_buffer);
        let mut unprocessed_grapheme = None;
        for _ in 0..rows {
            let mut graphemes = Vec::with_capacity(cols);
            let mut positions = Vec::with_capacity(cols);
            let mut width_remaining = cols;

            // Fill `graphemes`.
            //
            // If we have a grapheme next to the last character of the last row,
            // specifically `unprocessed_grapheme` is Some, avoid consuming
            // the grapheme iterator.
            while let Some(grapheme_rope) =
                unprocessed_grapheme.take().or_else(|| grapheme_iter.next())
            {
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
                        // Go to the next line.
                        pos.y += 1;
                        pos.x = 0;
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
                };
            }

            self.rows.push(DisplayRow {
                graphemes,
                positions,
            });
        }
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

    #[test]
    fn test_layout() {
        let mut view = View::new(Highlighter::new(&PLAIN));

        let buffer = Buffer::from_text("ABC\nX\nY");
        view.layout(&buffer, 3, 5);
        assert_eq!(view.rows().len(), 3);
        assert_eq!(view.rows()[0].graphemes, vec![g("A"), g("B"), g("C")]);
        assert_eq!(view.rows()[1].graphemes, vec![g("X")]);
        assert_eq!(view.rows()[2].graphemes, vec![g("Y")]);

        // Soft wrapping.
        let buffer = Buffer::from_text("ABC123XYZ");
        view.layout(&buffer, 2, 3);
        assert_eq!(view.rows().len(), 2);
        assert_eq!(view.rows()[0].graphemes, vec![g("A"), g("B"), g("C")]);
        assert_eq!(view.rows()[1].graphemes, vec![g("1"), g("2"), g("3")]);
    }
}
