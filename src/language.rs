pub struct Keywords {
    pub ctrls: &'static [&'static str],
    pub defs: &'static [&'static str],
}

pub struct Language {
    pub line_comments: &'static [&'static str],
    pub keywords: Keywords,
    pub strings: &'static [(
            &'static str /* start */,
            &'static str /* end */,
            Option<&'static str> /* escape */,
        )],
}

pub const CXX: Language = Language {
    line_comments: &["//"],
    keywords: Keywords {
        ctrls: &[
            "if", "for", "while", "do", "goto", "break", "continue", "case",
            "default", "return", "switch"
        ],
        defs: &[
            "typedef", "enum", "struct", "union", "class", "namespace", "using",
        ],
    },
    strings: &[("\"", "\"", Some("\\\""))],
};
