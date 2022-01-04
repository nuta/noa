use std::hash::{Hash, Hasher};

use crate::lsp::Lsp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxSpanType {
    MatchedBySearch,
    Comment,
    Ident,
    StringLiteral,
    EscapeSequence,
    PrimitiveType,
    CMacro,
    CIncludeArg,
}

pub struct Language {
    pub id: &'static str,
    pub filenames: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub formatter: Option<&'static [&'static str]>,
    pub lsp: Option<Lsp>,
    pub tree_sitter_mapping: phf::Map<&'static str, SyntaxSpanType>,
    pub(crate) tree_sitter_lib: Option<unsafe extern "C" fn() -> tree_sitter::Language>,
}

impl Language {
    pub fn syntax_highlighting_parser(&self) -> Option<tree_sitter::Parser> {
        self.tree_sitter_lib.as_ref().and_then(|lib| {
            let mut parser = tree_sitter::Parser::new();
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

impl Hash for Language {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for Language {
    fn eq(&self, other: &Language) -> bool {
        self.id == other.id
    }
}

impl Eq for Language {}
