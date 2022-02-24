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

pub fn get_highlight_query(name: &str) -> Option<&str> {
   match name {
    _ => None
    }
}
