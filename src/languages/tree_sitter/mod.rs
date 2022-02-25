pub use tree_sitter::*;
extern "C" {
    fn tree_sitter_rust() -> Language;
    fn tree_sitter_c() -> Language;
}

pub fn get_tree_sitter_parser(name: &str) -> Option<Language> {
   match name {
        "rust" => Some(unsafe { tree_sitter_rust() }),
        "c" => Some(unsafe { tree_sitter_c() }),
    _ => None
    }
}

pub fn get_highlights_query(name: &str) -> Option<&str> {
   match name {
        "rust" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/rust/highlights.scm")),
        "c" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/c/highlights.scm")),
    _ => None
    }
}
pub fn get_indents_query(name: &str) -> Option<&str> {
   match name {
        "rust" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/rust/indents.scm")),
        "c" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/c/indents.scm")),
    _ => None
    }
}
