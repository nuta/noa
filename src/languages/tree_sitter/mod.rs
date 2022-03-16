pub use tree_sitter::*;
extern "C" {
    fn tree_sitter_rust() -> Language;
    fn tree_sitter_c() -> Language;
    fn tree_sitter_cpp() -> Language;
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
        "rust" => Some(unsafe { tree_sitter_rust() }),
        "c" => Some(unsafe { tree_sitter_c() }),
        "cpp" => Some(unsafe { tree_sitter_cpp() }),
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

pub fn get_highlights_query(name: &str) -> Option<&str> {
   match name {
        "rust" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/rust/highlights.scm"),
        )),
        "c" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/c/highlights.scm"),
        )),
        "cpp" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/c/highlights.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/cpp/highlights.scm"),
        )),
        "javascript" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/ecma/highlights.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/jsx/highlights.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/javascript/highlights.scm"),
        )),
        "python" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/python/highlights.scm"),
        )),
        "go" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/go/highlights.scm"),
        )),
        "bash" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/bash/highlights.scm"),
        )),
        "html" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/html_tags/highlights.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/html/highlights.scm"),
        )),
        "css" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/css/highlights.scm"),
        )),
        "scss" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/css/highlights.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/scss/highlights.scm"),
        )),
        "typescript" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/ecma/highlights.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/typescript/highlights.scm"),
        )),
        "tsx" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/typescript/highlights.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/jsx/highlights.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/tsx/highlights.scm"),
        )),
        "markdown" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/markdown/highlights.scm"),
        )),
        "toml" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/toml/highlights.scm"),
        )),
        "json" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/json/highlights.scm"),
        )),
        "yaml" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/yaml/highlights.scm"),
        )),
        "make" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/make/highlights.scm"),
        )),
        "dockerfile" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/dockerfile/highlights.scm"),
        )),
        "regex" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/regex/highlights.scm"),
        )),
        "comment" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/comment/highlights.scm"),
        )),
    _ => None
    }
}
pub fn get_indents_query(name: &str) -> Option<&str> {
   match name {
        "rust" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/rust/indents.scm"),
        )),
        "c" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/c/indents.scm"),
        )),
        "cpp" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/c/indents.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/cpp/indents.scm"),
        )),
        "javascript" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/ecma/indents.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/jsx/indents.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/javascript/indents.scm"),
        )),
        "python" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/python/indents.scm"),
        )),
        "go" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/go/indents.scm"),
        )),
        "html" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/html_tags/indents.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/html/indents.scm"),
        )),
        "css" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/css/indents.scm"),
        )),
        "scss" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/css/indents.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/scss/indents.scm"),
        )),
        "typescript" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/ecma/indents.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/typescript/indents.scm"),
        )),
        "tsx" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/typescript/indents.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/jsx/indents.scm"),
            include_str!("../tree_sitter/nvim_treesitter/queries/tsx/indents.scm"),
        )),
        "toml" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/toml/indents.scm"),
        )),
        "json" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/json/indents.scm"),
        )),
        "yaml" => Some(concat!(
            include_str!("../tree_sitter/nvim_treesitter/queries/yaml/indents.scm"),
        )),
    _ => None
    }
}
