pub use tree_sitter::*;
extern "C" {
    fn tree_sitter_plain() -> Language;
    fn tree_sitter_c() -> Language;
    fn tree_sitter_cpp() -> Language;
    fn tree_sitter_rust() -> Language;
    fn tree_sitter_javascript() -> Language;
    fn tree_sitter_python() -> Language;
    fn tree_sitter_go() -> Language;
    fn tree_sitter_bash() -> Language;
    fn tree_sitter_html() -> Language;
    fn tree_sitter_css() -> Language;
    fn tree_sitter_scss() -> Language;
    fn tree_sitter_typescript() -> Language;
    fn tree_sitter_tsx() -> Language;
    fn tree_sitter_markdown() -> Language;
    fn tree_sitter_toml() -> Language;
    fn tree_sitter_json() -> Language;
    fn tree_sitter_yaml() -> Language;
    fn tree_sitter_make() -> Language;
    fn tree_sitter_dockerfile() -> Language;
    fn tree_sitter_regex() -> Language;
    fn tree_sitter_comment() -> Language;
}

pub fn get_tree_sitter_parser(name: &str) -> Option<Language> {
   match name {
        "plain" => Some(unsafe { tree_sitter_plain() }),
        "c" => Some(unsafe { tree_sitter_c() }),
        "cpp" => Some(unsafe { tree_sitter_cpp() }),
        "rust" => Some(unsafe { tree_sitter_rust() }),
        "javascript" => Some(unsafe { tree_sitter_javascript() }),
        "python" => Some(unsafe { tree_sitter_python() }),
        "go" => Some(unsafe { tree_sitter_go() }),
        "bash" => Some(unsafe { tree_sitter_bash() }),
        "html" => Some(unsafe { tree_sitter_html() }),
        "css" => Some(unsafe { tree_sitter_css() }),
        "scss" => Some(unsafe { tree_sitter_scss() }),
        "typescript" => Some(unsafe { tree_sitter_typescript() }),
        "tsx" => Some(unsafe { tree_sitter_tsx() }),
        "markdown" => Some(unsafe { tree_sitter_markdown() }),
        "toml" => Some(unsafe { tree_sitter_toml() }),
        "json" => Some(unsafe { tree_sitter_json() }),
        "yaml" => Some(unsafe { tree_sitter_yaml() }),
        "make" => Some(unsafe { tree_sitter_make() }),
        "dockerfile" => Some(unsafe { tree_sitter_dockerfile() }),
        "regex" => Some(unsafe { tree_sitter_regex() }),
        "comment" => Some(unsafe { tree_sitter_comment() }),
    _ => None
    }
}

pub fn get_highlight_query(name: &str) -> Option<&str> {
   match name {
        "plain" => Some(unsafe { include_str!("nvim_treesitter/queries/plain/highlight.scm") }),
        "c" => Some(unsafe { include_str!("nvim_treesitter/queries/c/highlight.scm") }),
        "cpp" => Some(unsafe { include_str!("nvim_treesitter/queries/cpp/highlight.scm") }),
        "rust" => Some(unsafe { include_str!("nvim_treesitter/queries/rust/highlight.scm") }),
        "javascript" => Some(unsafe { include_str!("nvim_treesitter/queries/javascript/highlight.scm") }),
        "python" => Some(unsafe { include_str!("nvim_treesitter/queries/python/highlight.scm") }),
        "go" => Some(unsafe { include_str!("nvim_treesitter/queries/go/highlight.scm") }),
        "bash" => Some(unsafe { include_str!("nvim_treesitter/queries/bash/highlight.scm") }),
        "html" => Some(unsafe { include_str!("nvim_treesitter/queries/html/highlight.scm") }),
        "css" => Some(unsafe { include_str!("nvim_treesitter/queries/css/highlight.scm") }),
        "scss" => Some(unsafe { include_str!("nvim_treesitter/queries/scss/highlight.scm") }),
        "typescript" => Some(unsafe { include_str!("nvim_treesitter/queries/typescript/highlight.scm") }),
        "tsx" => Some(unsafe { include_str!("nvim_treesitter/queries/tsx/highlight.scm") }),
        "markdown" => Some(unsafe { include_str!("nvim_treesitter/queries/markdown/highlight.scm") }),
        "toml" => Some(unsafe { include_str!("nvim_treesitter/queries/toml/highlight.scm") }),
        "json" => Some(unsafe { include_str!("nvim_treesitter/queries/json/highlight.scm") }),
        "yaml" => Some(unsafe { include_str!("nvim_treesitter/queries/yaml/highlight.scm") }),
        "make" => Some(unsafe { include_str!("nvim_treesitter/queries/make/highlight.scm") }),
        "dockerfile" => Some(unsafe { include_str!("nvim_treesitter/queries/dockerfile/highlight.scm") }),
        "regex" => Some(unsafe { include_str!("nvim_treesitter/queries/regex/highlight.scm") }),
        "comment" => Some(unsafe { include_str!("nvim_treesitter/queries/comment/highlight.scm") }),
    _ => None
    }
}
