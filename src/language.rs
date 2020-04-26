pub struct Keywords {
    pub ctrls: &'static [&'static str],
    pub defs: &'static [&'static str],
}

pub struct Language {
    pub filenames: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub line_comments: &'static [&'static str],
    pub keywords: Keywords,
    pub strings: &'static [(
            &'static str /* start */,
            &'static str /* end */,
            Option<&'static str> /* escape */,
        )],
}

pub const LANGS: &'static [Language] = &[PLAIN, CXX];

pub const PLAIN: Language = Language {
    filenames: &[],
    extensions: &[],
    line_comments: &[],
    strings: &[],
    keywords: Keywords {
        ctrls: &[],
        defs: &[],
    },
};

pub const CXX: Language = Language {
    filenames: &[],
    extensions: &["c", "h", "cxx", "cpp"],
    line_comments: &["//"],
    strings: &[("\"", "\"", Some("\\\""))],
    keywords: Keywords {
        ctrls: &[
            "if", "for", "while", "do", "goto", "break", "continue", "case",
            "default", "return", "switch"
        ],
        defs: &[
            "typedef", "enum", "struct", "union", "class", "namespace", "using",
        ],
    },
};
