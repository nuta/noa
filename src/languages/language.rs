use std::{
    hash::{Hash, Hasher},
    str::FromStr,
};

use crate::{lsp::Lsp, tree_sitter::TreeSitter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxSpan {
    Comment,
    Ident,
    Control,
    StringLiteral,
    EscapeSequence,
    PrimitiveType,
    CMacro,
    CIncludeArg,
}

impl FromStr for SyntaxSpan {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let span = match s {
            "comment" => SyntaxSpan::Comment,
            "ident" => SyntaxSpan::Ident,
            "keyword.control" => SyntaxSpan::Control,
            "string" => SyntaxSpan::StringLiteral,
            "escape" => SyntaxSpan::EscapeSequence,
            "primitive" => SyntaxSpan::PrimitiveType,
            "cmacro" => SyntaxSpan::CMacro,
            "cincludearg" => SyntaxSpan::CIncludeArg,
            _ => {
                return Err(());
            }
        };

        Ok(span)
    }
}

pub struct Language {
    pub id: &'static str,
    pub filenames: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub formatter: Option<&'static [&'static str]>,
    pub lsp: Option<Lsp>,
    pub tree_sitter: Option<TreeSitter>,
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
