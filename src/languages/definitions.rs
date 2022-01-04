use phf::phf_map;

use crate::language::{Language, SyntaxSpanType};
use crate::lsp::Lsp;
use crate::tree_sitter;

pub const LANGUAGES: &[Language] = &[PLAIN, C];

pub const PLAIN: Language = Language {
    id: "plain",
    filenames: &[],
    extensions: &[],
    formatter: None,
    lsp: None,
    tree_sitter_lib: None,
    tree_sitter_mapping: phf_map! {},
};

pub const C: Language = Language {
    id: "c",
    filenames: &[],
    extensions: &["c", "h"],
    formatter: Some(&["clang-format"]),
    lsp: Some(Lsp {
        language_id: "c",
        command: &["clangd", "-j=8", "--log=verbose", "--pretty"],
    }),
    tree_sitter_lib: Some(tree_sitter::tree_sitter_c),
    tree_sitter_mapping: phf_map! {
        "comment" => SyntaxSpanType::Comment,
        "identifier" => SyntaxSpanType::Ident,
        "string_literal" => SyntaxSpanType::StringLiteral,
        "primitive_type" => SyntaxSpanType::PrimitiveType,
        "escape_sequence" => SyntaxSpanType::EscapeSequence,
        "preproc_include" => SyntaxSpanType::CMacro,
        "#include" => SyntaxSpanType::CMacro,
        "system_lib_string" => SyntaxSpanType::CIncludeArg,
    },
};
