pub use tree_sitter::*;

extern "C" {
    pub fn tree_sitter_c() -> Language;
    // pub fn tree_sitter_cpp() -> Language;
    pub fn tree_sitter_rust() -> Language;
}

pub struct TreeSitter {
    pub get_language: fn() -> tree_sitter::Language,
    pub highlight_query: &'static str,
}
