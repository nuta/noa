pub use tree_sitter::*;
extern "C" {
    fn tree_sitter_rust() -> Language;
    fn tree_sitter_c() -> Language;
    fn tree_sitter_javascript() -> Language;
    fn tree_sitter_go() -> Language;
    fn tree_sitter_css() -> Language;
    fn tree_sitter_scss() -> Language;
    fn tree_sitter_typescript() -> Language;
    fn tree_sitter_tsx() -> Language;
    fn tree_sitter_toml() -> Language;
    fn tree_sitter_json() -> Language;
    fn tree_sitter_make() -> Language;
    fn tree_sitter_dockerfile() -> Language;
    fn tree_sitter_regex() -> Language;
    fn tree_sitter_comment() -> Language;
}

pub fn get_tree_sitter_parser(name: &str) -> Option<Language> {
   match name {
        "rust" => Some(unsafe { tree_sitter_rust() }),
        "c" => Some(unsafe { tree_sitter_c() }),
        "javascript" => Some(unsafe { tree_sitter_javascript() }),
        "go" => Some(unsafe { tree_sitter_go() }),
        "css" => Some(unsafe { tree_sitter_css() }),
        "scss" => Some(unsafe { tree_sitter_scss() }),
        "typescript" => Some(unsafe { tree_sitter_typescript() }),
        "tsx" => Some(unsafe { tree_sitter_tsx() }),
        "toml" => Some(unsafe { tree_sitter_toml() }),
        "json" => Some(unsafe { tree_sitter_json() }),
        "make" => Some(unsafe { tree_sitter_make() }),
        "dockerfile" => Some(unsafe { tree_sitter_dockerfile() }),
        "regex" => Some(unsafe { tree_sitter_regex() }),
        "comment" => Some(unsafe { tree_sitter_comment() }),
    _ => None
    }
}

pub fn get_highlights_query(name: &str) -> Option<&str> {
   match name {
        "rust" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/rust/highlights.scm")),
        "c" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/c/highlights.scm")),
        "javascript" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/javascript/highlights.scm")),
        "go" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/go/highlights.scm")),
        "css" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/css/highlights.scm")),
        "scss" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/scss/highlights.scm")),
        "typescript" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/typescript/highlights.scm")),
        "tsx" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/tsx/highlights.scm")),
        "toml" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/toml/highlights.scm")),
        "json" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/json/highlights.scm")),
        "make" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/make/highlights.scm")),
        "dockerfile" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/dockerfile/highlights.scm")),
        "regex" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/regex/highlights.scm")),
        "comment" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/comment/highlights.scm")),
    _ => None
    }
}
pub fn get_indents_query(name: &str) -> Option<&str> {
   match name {
        "rust" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/rust/indents.scm")),
        "c" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/c/indents.scm")),
        "javascript" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/javascript/indents.scm")),
        "go" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/go/indents.scm")),
        "css" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/css/indents.scm")),
        "scss" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/scss/indents.scm")),
        "typescript" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/typescript/indents.scm")),
        "tsx" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/tsx/indents.scm")),
        "toml" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/toml/indents.scm")),
        "json" => Some(include_str!("../tree_sitter/nvim_treesitter/queries/json/indents.scm")),
    _ => None
    }
}
