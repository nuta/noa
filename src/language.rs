use regex::Regex;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SpanType {
    Cursor,
    Selection,
    Normal,
    StringLiteral,
    EscapedChar,
    Comment,
    CtrlKeyword,
}

pub enum Pattern {
    Inline {
        regex: Regex,
        captures: &'static [SpanType],
    },
    Block {
        start: Regex,
        end: Regex,
        start_captures: &'static [SpanType],
        end_captures: &'static [SpanType],
        inner: SpanType,
        patterns: &'static [&'static str],
    },
}

pub struct LspSettings {
    pub language_id: &'static str,
    pub command: &'static [&'static str],
}

pub struct Language {
    pub name: &'static str,
    pub lsp: Option<&'static LspSettings>,
    pub patterns: HashMap<&'static str, Pattern>,
    pub top_level_patterns: &'static [&'static str],
}

impl Hash for Language {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for Language {
    fn eq(&self, other: &Language) -> bool {
        self.name == other.name
    }
}

impl Eq for Language {}

lazy_static! {
    pub static ref PLAIN: Language = {
        Language {
            name: "plain",
            top_level_patterns: &[
                "block_comment",
                "ctrl",
                "string_lit",
            ],
            lsp: Some(&LspSettings {
                language_id: "c",
                command: &["clangd", "-j=8", "--log=verbose", "--pretty"],
            }),
            patterns: hashmap! {
                "block_comment" => Pattern::Block {
                    start: Regex::new(r"(/\*)").unwrap(),
                    end: Regex::new(r"(\*/)").unwrap(),
                    start_captures: &[SpanType::Comment],
                    end_captures: &[SpanType::Comment],
                    inner: SpanType::Comment,
                    patterns: &[],
                },
                "ctrl" => Pattern::Inline {
                    regex: Regex::new(
                        concat!(
                            r"\b(if|for|while|do|goto|break|continue|case|",
                            r"default|return|switch)\b",
                        )
                    ).unwrap(),
                    captures: &[
                        SpanType::CtrlKeyword,
                    ]
                },
                "escaped_chars" => Pattern::Inline {
                    regex: Regex::new(
                        "(\\\\[\"tn])"
                    ).unwrap(),
                    captures: &[
                        SpanType::EscapedChar,
                    ]
                },
                "string_lit" => Pattern::Block {
                    start: Regex::new("(\")").unwrap(),
                    end: Regex::new("(\")").unwrap(),
                    start_captures: &[SpanType::StringLiteral],
                    end_captures: &[SpanType::StringLiteral],
                    inner: SpanType::StringLiteral,
                    patterns: &[
                        "escaped_chars",
                    ],
                },
            },
        }
    };
}
