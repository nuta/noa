use arrayvec::ArrayString;
use noa_buffer::{
    buffer::Buffer,
    cursor::{Position, Range},
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

    pub fn layout(&mut self, buffer: &Buffer, rows: std::ops::Range<usize>, cols: usize) {
        self.rows.clear();
        let mut chars = buffer.grapheme_iter(Range::new(0, 0, buffer.num_lines(), 0));
        for _ in 0..rows.end {
            // self.rows.push(DisplayRow {});
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
