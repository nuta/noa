pub struct Language {
    pub line_comments: &'static [&'static str],
    pub ctrl_stmts: &'static [&'static str],
}

pub const C: Language = Language {
    line_comments: &["//"],
    ctrl_stmts: &["if"],
};
