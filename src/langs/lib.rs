#[allow(unused_imports)]
#[macro_use]
extern crate log;

use std::hash::{Hash, Hasher};

pub mod tree_sitter;

pub struct Lsp {
    pub language_id: &'static str,
    pub command: &'static [&'static str],
}

pub struct Lang {
    pub id: &'static str,
    pub filenames: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub lsp: Option<Lsp>,
    tree_sitter_lib: Option<unsafe extern "C" fn() -> ::tree_sitter::Language>,
}

impl Lang {
    pub fn tree_sitter(&self) -> Option<::tree_sitter::Parser> {
        self.tree_sitter_lib.as_ref().and_then(|lib| {
            let mut parser = ::tree_sitter::Parser::new();
            match parser.set_language(unsafe { lib() }) {
                Ok(()) => Some(parser),
                Err(err) => {
                    error!("failed to load tree sitter for {}: {}", self.id, err);
                    None
                }
            }
        })
    }
}

impl Hash for Lang {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for Lang {
    fn eq(&self, other: &Lang) -> bool {
        self.id == other.id
    }
}

impl Eq for Lang {}

pub const LANGS: &[Lang] = &[PLAIN, C];

pub const PLAIN: Lang = Lang {
    id: "plain",
    filenames: &[],
    extensions: &[],
    lsp: None,
    tree_sitter_lib: None,
};

pub const C: Lang = Lang {
    id: "c",
    filenames: &[],
    extensions: &["c", "h"],
    lsp: Some(Lsp {
        language_id: "c",
        command: &["clangd", "-j=8", "--log=verbose", "--pretty"],
    }),
    tree_sitter_lib: Some(tree_sitter::tree_sitter_c),
};
