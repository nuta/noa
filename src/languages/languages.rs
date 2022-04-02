//! Language definitions.
//!
//! This file must be independent: it must not depend on any other crate except
//! `std` because it's also used from `build.rs`.
use std::hash::{Hash, Hasher};

pub struct TreeSitter {
    pub dir: Option<&'static str>,
    pub url: &'static str,
    pub sources: &'static [&'static str],
}

pub struct Lsp {
    pub identifier: &'static str,
    pub argv: &'static [&'static str],
    pub envp: &'static [(&'static str, &'static str)],
}

pub struct Language {
    pub name: &'static str,
    pub filenames: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub line_comment: Option<&'static str>,
    pub heutristic_search_regex: Option<&'static str>,
    pub tree_sitter: Option<TreeSitter>,
    pub lsp: Option<Lsp>,
}

impl Hash for Language {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for Language {
    fn eq(&self, other: &Language) -> bool {
        self.name == other.name
    }
}

impl Eq for Language {}

pub static LANGUAGES: &[Language] = &[
    Language {
        name: "plain",
        filenames: &[],
        extensions: &[],
        line_comment: None,
        heutristic_search_regex: None,
        tree_sitter: None,
        lsp: None,
    },
    Language {
        name: "rust",
        filenames: &[],
        extensions: &["rs"],
        line_comment: Some("//"),
        heutristic_search_regex: Some(r"(type|struct|static|const|fn)\w\1"),
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-rust",
            sources: &["src/parser.c", "src/scanner.c"],
            dir: None,
        }),
        lsp: Some(Lsp {
            identifier: "rust",
            argv: &["rust-analyzer"],
            envp: &[("RA_LOG", "verbose")],
        }),
    },
    Language {
        name: "c",
        filenames: &[],
        extensions: &["c", "h"],
        line_comment: Some("//"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-c",
            sources: &["src/parser.c"],
            dir: None,
        }),
        lsp: Some(Lsp {
            identifier: "c",
            argv: &["clangd", "--pretty", "--log=verbose"],
            envp: &[],
        }),
    },
    Language {
        name: "cpp",
        filenames: &[],
        extensions: &["cpp", "cxx", "hpp", "hxx"],
        line_comment: Some("//"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-cpp",
            sources: &["src/parser.c", "src/scanner.cc"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "javascript",
        filenames: &[],
        extensions: &["js"],
        line_comment: Some("//"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-javascript",
            sources: &["src/parser.c", "src/scanner.c"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "python",
        filenames: &[],
        extensions: &["py"],
        line_comment: Some("#"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-python",
            sources: &["src/parser.c", "src/scanner.cc"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "go",
        filenames: &[],
        extensions: &["go"],
        line_comment: Some("//"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-go",
            sources: &["src/parser.c"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "bash",
        filenames: &[],
        extensions: &["sh", "bash"],
        line_comment: Some("#"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-bash",
            sources: &["src/parser.c", "src/scanner.cc"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "html",
        filenames: &[],
        extensions: &["html"],
        line_comment: None,
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-html",
            sources: &["src/parser.c", "src/scanner.cc"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "css",
        filenames: &[],
        extensions: &["css"],
        line_comment: None,
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-css",
            sources: &["src/parser.c", "src/scanner.c"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "scss",
        filenames: &[],
        extensions: &["scss"],
        line_comment: Some("//"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/serenadeai/tree-sitter-scss",
            sources: &["src/parser.c", "src/scanner.c"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "typescript",
        filenames: &[],
        extensions: &["ts"],
        line_comment: Some("//"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-typescript",
            sources: &["src/parser.c", "src/scanner.c"],
            dir: Some("typescript"),
        }),
        lsp: None,
    },
    Language {
        name: "tsx",
        filenames: &[],
        extensions: &["tsx"],
        line_comment: Some("//"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-typescript",
            sources: &["src/parser.c", "src/scanner.c"],
            dir: Some("tsx"),
        }),
        lsp: None,
    },
    Language {
        name: "markdown",
        filenames: &[],
        extensions: &["md"],
        line_comment: None,
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/MDeiml/tree-sitter-markdown",
            sources: &["src/parser.c", "src/scanner.cc"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "toml",
        filenames: &[],
        extensions: &["toml"],
        line_comment: Some("#"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/ikatyang/tree-sitter-toml",
            sources: &["src/parser.c", "src/scanner.c"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "json",
        filenames: &[],
        extensions: &["json"],
        line_comment: None,
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-json",
            sources: &["src/parser.c"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "yaml",
        filenames: &[],
        extensions: &["yml", "yaml"],
        line_comment: Some("#"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/ikatyang/tree-sitter-yaml",
            sources: &["src/parser.c", "src/scanner.cc"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "make",
        filenames: &["Makefile"],
        extensions: &["mk", "makefile"],
        line_comment: Some("#"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/alemuller/tree-sitter-make",
            sources: &["src/parser.c"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "dockerfile",
        filenames: &["Dockerfile"],
        extensions: &["dockerfile"],
        line_comment: Some("#"),
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/camdencheek/tree-sitter-dockerfile",
            sources: &["src/parser.c"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "regex",
        filenames: &[],
        extensions: &[],
        line_comment: None,
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/tree-sitter/tree-sitter-regex",
            sources: &["src/parser.c"],
            dir: None,
        }),
        lsp: None,
    },
    Language {
        name: "comment",
        filenames: &[],
        extensions: &[],
        line_comment: None,
        heutristic_search_regex: None,
        tree_sitter: Some(TreeSitter {
            url: "https://github.com/stsewd/tree-sitter-comment",
            sources: &["src/parser.c", "src/scanner.c"],
            dir: None,
        }),
        lsp: None,
    },
];
