use crate::highlight::Style;
use crate::language::{SpanType, Pattern, Language};
use crate::status_map::LineStatusType;
use crossterm::style::{Attribute, Color};
use std::collections::HashMap;
use std::io::Stdout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeItem {
    Span(SpanType),
    LineNo,
    LineNoPadding,
    LineStatus(LineStatusType),
    LineStatusPadding,
    CommandBoxPrompt,
    ScrollBarVisible,
    PopupItemHover,
    PopupItem,
    InfoBarColor,
    DirtyBufferMark,
}

pub struct Theme {
    items: HashMap<ThemeItem, Style>,
}

impl Theme {
    pub fn from_hash_map(items: HashMap<ThemeItem, Style>) -> Theme {
        Theme {
            items,
        }
    }

    pub fn apply(&self, stdout: &mut Stdout, item: ThemeItem) -> crossterm::Result<()> {
        self.items[&item].apply(stdout)
    }

    pub fn apply_span(&self, stdout: &mut Stdout, span: SpanType) -> crossterm::Result<()> {
        self.apply(stdout, ThemeItem::Span(span))
    }
}

// The default theme.
lazy_static! {
    pub static ref THEME: Theme = {
        let items = hashmap! {
            ThemeItem::Span(SpanType::Cursor) => Style::inverted(),
            ThemeItem::Span(SpanType::Selection) => Style::inverted(),
            ThemeItem::Span(SpanType::Normal) => Style::normal(),
            ThemeItem::Span(SpanType::StringLiteral) => Style::fg(Color::Cyan),
            ThemeItem::Span(SpanType::EscapedChar) => Style::fg(Color::Cyan),
            ThemeItem::Span(SpanType::Comment) => Style::fg(Color::Cyan),
            ThemeItem::Span(SpanType::CtrlKeyword) => Style::fg(Color::Cyan),
            ThemeItem::CommandBoxPrompt => Style::fg(Color::Magenta),
            ThemeItem::LineNo => Style::fg(Color::AnsiValue(253)),
            ThemeItem::LineNoPadding => Style::fg(Color::AnsiValue(240)),
            ThemeItem::LineStatus(LineStatusType::Added) => Style::fg(Color::AnsiValue(66)),
            ThemeItem::LineStatus(LineStatusType::Deleted) => Style::fg(Color::AnsiValue(16)),
            ThemeItem::LineStatus(LineStatusType::Modified) => Style::fg(Color::AnsiValue(44)),
            ThemeItem::LineStatusPadding => Style::fg(Color::AnsiValue(123)),
            ThemeItem::CommandBoxPrompt => Style::fg(Color::AnsiValue(12)),
            ThemeItem::ScrollBarVisible => Style::fg(Color::AnsiValue(153)),
            ThemeItem::PopupItemHover => Style::fg(Color::AnsiValue(173)),
            ThemeItem::PopupItem => Style::fg(Color::AnsiValue(183)),
            ThemeItem::InfoBarColor => Style::fg(Color::AnsiValue(93)),
            ThemeItem::DirtyBufferMark => Style::fg(Color::AnsiValue(223)),

        };

        Theme::from_hash_map(items)
    };
}
