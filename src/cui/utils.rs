use std::cmp::min;

pub fn whitespaces(n: usize) -> String {
    " ".repeat(n)
}

pub fn truncate_to_width(s: &str, width: usize) -> &str {
    // TODO: Support CJK using DisplayWidth
    &s[..min(s.chars().count(), width)]
}
