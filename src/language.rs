pub struct Language {
    pub line_comments: &'static [&'static str],
    pub ctrl_stmts: &'static [&'static str],
    pub strings: &'static [(
            &'static str /* start */,
            &'static str /* end */,
            Option<&'static str> /* escape */,
        )],
}

pub const C: Language = Language {
    line_comments: &["//"],
    ctrl_stmts: &["if"],
    strings: &[("\"", "\"", Some("\\\""))],
};
