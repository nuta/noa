use std::hash::{Hash, Hasher};

pub struct Keywords {
    pub ctrls: &'static [&'static str],
    pub defs: &'static [&'static str],
}

pub struct Lsp {
    pub language_id: &'static str,
    pub command: &'static [&'static str],
}

pub struct Language {
    pub id: &'static str,
    pub filenames: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub lsp: Option<Lsp>,
    pub line_comments: &'static [&'static str],
    pub keywords: Keywords,
    pub strings: &'static [(
            &'static str /* start */,
            &'static str /* end */,
            Option<&'static str> /* escape */,
        )],
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

pub const LANGS: &'static [Language] = &[PLAIN, C];

pub const PLAIN: Language = Language {
    id: "plain",
    filenames: &[],
    extensions: &[],
    lsp: None,
    line_comments: &[],
    strings: &[],
    keywords: Keywords {
        ctrls: &[],
        defs: &[],
    },
};

pub const C: Language = Language {
    id: "c",
    filenames: &[],
    extensions: &["c", "h"],
    line_comments: &["//"],
    lsp: Some(Lsp {
        language_id: "c",
        command: &["clangd", "-j=8", "--log=verbose", "--pretty"]
    }),
    strings: &[("\"", "\"", Some("\\\""))],
    keywords: Keywords {
        ctrls: &[
            "if", "for", "while", "do", "goto", "break", "continue", "case",
            "default", "return", "switch"
        ],
        defs: &[
            "typedef", "enum", "struct", "union",
        ],
    },
};
