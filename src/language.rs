pub struct Language {
    pub id: &'static str,
    pub filenames: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub line_comments: &'static [&'static str],
    pub keywords: &'static [&'static str],
    pub strings: &'static [(
            &'static str /* start */,
            &'static str /* end */,
            Option<char> /* escape */,
        )],
}

pub const LANGS: &'static [Language] = &[
    PLAIN,
    C,
];

pub const PLAIN: Language = Language {
    id: "plain",
    filenames: &[],
    extensions: &[],
    line_comments: &[],
    strings: &[],
    keywords: &[],
};

pub const C: Language = Language {
    id: "c",
    filenames: &[],
    extensions: &["c", "h"],
    line_comments: &["//"],
    strings: &[("\"", "\"", Some('\\'))],
    keywords: &[
        "if", "for", "while", "do", "goto", "break", "continue", "case",
        "default", "return", "switch"
    ],
};
