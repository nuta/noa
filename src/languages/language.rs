use std::hash::{Hash, Hasher};

use crate::lsp::Lsp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxSpanType {
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
    pub tree_sitter_language: Option<fn() -> tree_sitter::Language>,
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
