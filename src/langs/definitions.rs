use phf::phf_map;

use crate::highlighting::HighlightType;
use crate::language::Language;
use crate::lsp::Lsp;
use crate::tree_sitter;

pub const LANGS: &[Language] = &[PLAIN, C];

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
        "comment" => HighlightType::Comment,
        "identifier" => HighlightType::Ident,
        "string_literal" => HighlightType::StringLiteral,
        "primitive_type" => HighlightType::PrimitiveType,
        "escape_sequence" => HighlightType::EscapeSequence,
        "preproc_include" => HighlightType::CMacro,
        "#include" => HighlightType::CMacro,
        "system_lib_string" => HighlightType::CIncludeArg,
    },
};
