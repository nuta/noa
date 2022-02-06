use std::ffi::OsStr;
use std::path::Path;

use phf::phf_map;

use crate::language::{Language, SyntaxSpan};
use crate::lsp::Lsp;
use crate::tree_sitter::*;

pub const LANGUAGES: &[Language] = &[PLAIN, C];

pub fn guess_language(path: &Path) -> &'static Language {
    for lang in LANGUAGES {
        for ext in lang.extensions {
            if path.extension() == Some(OsStr::new(*ext)) {
                return lang;
            }
        }
    }

    &PLAIN
}

pub const PLAIN: Language = Language {
    id: "plain",
    filenames: &[],
    extensions: &[],
    formatter: None,
    lsp: None,
    tree_sitter_language: None,
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
    tree_sitter_language: Some(|| unsafe { tree_sitter_c() }),
    tree_sitter_mapping: phf_map! {
        "comment" => SyntaxSpan::Comment,
        "identifier" => SyntaxSpan::Ident,
        "string_literal" => SyntaxSpan::StringLiteral,
        "primitive_type" => SyntaxSpan::PrimitiveType,
        "escape_sequence" => SyntaxSpan::EscapeSequence,
        "preproc_include" => SyntaxSpan::CMacro,
        "#include" => SyntaxSpan::CMacro,
        "system_lib_string" => SyntaxSpan::CIncludeArg,
    },
};

pub const RUST: Language = Language {
    id: "rust",
    filenames: &[],
    extensions: &["rs"],
    formatter: None,
    lsp: None,
    tree_sitter_language: Some(|| unsafe { tree_sitter_rust() }),
    tree_sitter_mapping: phf_map! {
        "comment" => SyntaxSpan::Comment,
    },
};
