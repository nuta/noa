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

pub struct DisplayRow {
    /// The graphemes in this row.
    graphemes: Vec<Grapheme>,
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

    pub fn update(&mut self, buffer: &Buffer) {
        self.highlighter.update(buffer);
    }

    /// Computes the grapheme layout.
    ///
    /// If you want to disable soft wrapping. Set `cols` to `std::max::MAX`.
    pub fn layout(&mut self, buffer: &Buffer, rows: std::ops::Range<usize>, cols: usize) {
        self.rows.clear();
        let mut grapheme_iter = buffer.grapheme_iter(Range::new(0, 0, buffer.num_lines(), 0));
        for _ in 0..rows.end {
            let mut graphemes = Vec::with_capacity(cols);
            let mut width_remaining = cols;
            while let Some(grapheme_rope) = grapheme_iter.next() {
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
                        break;
                    }
                    _ => {
                        let grapheme_width = chars.as_str().display_width();
                        if grapheme_width > width_remaining {
                            break;
                        }

                        width_remaining -= grapheme_width;
                        graphemes.push(Grapheme {
                            chars,
                            style: Style::default(),
                        });
                    }
                };
            }

            self.rows.push(DisplayRow { graphemes });
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
