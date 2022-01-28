use std::ffi::OsStr;
use std::path::Path;

use crate::language::Language;
use crate::lsp::Lsp;
use crate::tree_sitter::tree_sitter_c;

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
};
