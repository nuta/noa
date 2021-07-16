#[allow(unused_imports)]
#[macro_use]
extern crate log;

use std::hash::{Hash, Hasher};

use phf::phf_map;

pub mod tree_sitter;

pub struct Lsp {
    pub language_id: &'static str,
    pub command: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightType {
    MatchedBySearch,
    Comment,
    Ident,
    StringLiteral,
    EscapeSequence,
    PrimitiveType,
    CMacro,
    CIncludeArg,
}

pub struct Lang {
    pub id: &'static str,
    pub filenames: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub formatter: Option<&'static [&'static str]>,
    pub lsp: Option<Lsp>,
    pub tree_sitter_mapping: phf::Map<&'static str, HighlightType>,
    tree_sitter_lib: Option<unsafe extern "C" fn() -> tree_sitter::Language>,
}

impl Lang {
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
    formatter: None,
    lsp: None,
    tree_sitter_lib: None,
    tree_sitter_mapping: phf_map! {},
};

pub const C: Lang = Lang {
    id: "c",
    filenames: &[],
    extensions: &["c", "h"],
    formatter: Some(&["clang-format"]),
    lsp: Some(Lsp {
        language_id: "c",
        command: &["clangd", "-j=8", "--log=verbose", "--pretty"],
    }),
    tree_sitter_lib: Some(tree_sitter::tree_sitter_c),
    tree_sitter_mapping: phf_map! {
        "comment" => HighlightType::Comment,
        "identifier" => HighlightType::Ident,
        "string_literal" => HighlightType::StringLiteral,
        "primitive_type" => HighlightType::PrimitiveType,
        "escape_sequence" => HighlightType::EscapeSequence,
        "preproc_include" => HighlightType::CMacro,
        "#include" => HighlightType::CMacro,
        "system_lib_string" => HighlightType::CIncludeArg,
    },
};

pub fn get_lsp_config_by_lsp_lang_id(id: &str) -> Option<&'static Lsp> {
    LANGS
        .iter()
        .find(|lang| match lang.lsp.as_ref() {
            Some(lsp) => lsp.language_id == id,
            None => false,
        })
        .map(|lang| lang.lsp.as_ref().unwrap())
}
