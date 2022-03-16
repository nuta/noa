use std::{
    collections::HashMap,
    ffi::OsStr,
    hash::{Hash, Hasher},
    path::Path,
    str::FromStr,
};

use bincode::Decode;

use once_cell::sync::Lazy;

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
            "variable" => SyntaxSpan::Ident,
            "keyword" => SyntaxSpan::Control,
            "string" => SyntaxSpan::StringLiteral,
            _ => {
                return Err(());
            }
        };

        Ok(span)
    }
}

#[derive(Decode)]
pub struct TreeSitter {
    pub dir: String,
    pub url: String,
    pub sources: Vec<String>,
}

#[derive(Decode)]
pub struct Lsp {
    pub identifier: String,
    pub argv: Vec<String>,
    pub envp: HashMap<String, String>,
}

#[derive(Decode)]
pub struct Language {
    pub name: String,
    pub filenames: Vec<String>,
    pub extensions: Vec<String>,
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

#[derive(Decode)]
pub struct Languages {
    languages: Vec<Language>,
}

pub static LANGUAGES: Lazy<Languages> = Lazy::new(|| {
    bincode::decode_from_slice(
        include_bytes!("languages.bincode"),
        bincode::config::standard(),
    )
    .expect("failed to load languages.bincode")
    .0
});

fn languages() -> &'static [Language] {
    &LANGUAGES.languages
}

pub fn guess_language(path: &Path) -> Option<&'static Language> {
    for lang in languages() {
        for ext in &lang.filenames {
            if path.file_name() == Some(OsStr::new(ext)) {
                return Some(lang);
            }
        }

        for ext in &lang.extensions {
            if path.extension() == Some(OsStr::new(ext)) {
                return Some(lang);
            }
        }
    }

    None
}

pub fn get_language_by_name(name: &str) -> Option<&'static Language> {
    for lang in languages() {
        if lang.name == name {
            return Some(lang);
        }
    }

    None
}
