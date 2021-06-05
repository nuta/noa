use std::hash::{Hash, Hasher};

pub struct Lsp {
    pub language_id: &'static str,
    pub command: &'static [&'static str],
}

pub struct Lang {
    pub id: &'static str,
    pub filenames: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub lsp: Option<Lsp>,
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
};

pub const C: Lang = Lang {
    id: "c",
    filenames: &[],
    extensions: &["c", "h"],
    lsp: Some(Lsp {
        language_id: "c",
        command: &["clangd", "-j=8", "--log=verbose", "--pretty"],
    }),
};
