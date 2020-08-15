use regex::Regex;
use std::cmp::min;
use std::ops::RangeInclusive;
use std::collections::HashMap;
use ropey::RopeSlice;
use crossterm::style::{Attribute, Color};
use crate::buffer::Snapshot;
use crate::rope::{Rope, Range, Cursor};

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
}

pub struct Language {
    pub name: &'static str,
    pub patterns: HashMap<&'static str, Pattern>,
    pub top_level_patterns: &'static [&'static str],
}

lazy_static! {
    pub static ref PLAIN: Language = {
        Language {
            name: "plain",
            top_level_patterns: &[
                "keyword",
            ],
            patterns: hashmap! {
                "keyword" => Pattern::Inline {
                    regex: Regex::new(
                        concat!(
                            r"\b(if|for|while|do|goto|break|continue|case|",
                            r"default|return|switch)\b",
                        )
                    ).unwrap(),
                    captures: &[
                        SpanType::CtrlKeyword,
                    ]
                }
            },
        }
    };
}
