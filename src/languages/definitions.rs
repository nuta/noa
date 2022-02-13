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

pub fn get_language_by_lsp_id(language_id: &str) -> Option<&'static Lsp> {
    for lang in LANGUAGES {
        if let Some(lsp) = lang.lsp.as_ref() {
            if lsp.language_id == language_id {
                return Some(lsp);
            }
        }
    }

    None
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
        get_argv: || {
            vec![
                "clangd".into(),
                format!("-j={}", num_cpus::get()),
                "--log=verbose".into(),
                "--pretty".into(),
            ]
        },
        get_envp: std::vec::Vec::new,
    }),
    tree_sitter: None,
};

pub const RUST: Language = Language {
    id: "rust",
    filenames: &[],
    extensions: &["rs"],
    lsp: Some(Lsp {
        language_id: "rust",
        get_argv: || {
            // FIXME:
            vec!["/home/seiya/.vscode-server/data/User/globalStorage/matklad.rust-analyzer/rust-analyzer-x86_64-unknown-linux-gnu".into()]
        },
        get_envp: || vec![("RA_LOG".into(), "verbose".into())],
    }),
    tree_sitter: Some(TreeSitter {
        get_language: || unsafe { tree_sitter_rust() },
        highlight_query: include_str!("queries/rust/highlight.scm"),
    }),
};
