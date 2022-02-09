use std::ffi::OsStr;
use std::path::Path;

use crate::language::Language;
use crate::lsp::Lsp;
use crate::tree_sitter::*;

pub const LANGUAGES: &[Language] = &[PLAIN, C, RUST];

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
    lsp: None,
    tree_sitter: None,
};

pub const C: Language = Language {
    id: "c",
    filenames: &[],
    extensions: &["c", "h"],
    lsp: Some(Lsp {
        language_id: "c",
        command: &["clangd", "-j=8", "--log=verbose", "--pretty"],
    }),
    tree_sitter: None,
};

pub const RUST: Language = Language {
    id: "rust",
    filenames: &[],
    extensions: &["rs"],
    lsp: None,
    tree_sitter: Some(TreeSitter {
        get_language: || unsafe { tree_sitter_rust() },
        highlight_query: include_str!("queries/rust/highlight.scm"),
    }),
};
