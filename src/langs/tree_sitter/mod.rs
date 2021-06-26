use tree_sitter::{Language, Parser};

extern "C" {
    pub fn tree_sitter_c() -> Language;
    // pub fn tree_sitter_cpp() -> Language;
    // pub fn tree_sitter_rust() -> Language;
}
