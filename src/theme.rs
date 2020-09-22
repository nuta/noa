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

macro_rules! color {
    ($n:expr) => (Color::AnsiValue($n))
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
            ThemeItem::LineNo => Style::color(color!(0), color!(253)),
            ThemeItem::LineNoPadding => Style::color(color!(0), color!(240)),
            ThemeItem::LineStatus(LineStatusType::Added) => Style::color(color!(0), color!(66)),
            ThemeItem::LineStatus(LineStatusType::Deleted) => Style::color(color!(0), color!(16)),
            ThemeItem::LineStatus(LineStatusType::Modified) => Style::color(color!(0), color!(44)),
            ThemeItem::LineStatusPadding => Style::bg(color!(123)),
            ThemeItem::CommandBoxPrompt => Style::color(color!(0), color!(12)),
            ThemeItem::ScrollBarVisible => Style::bg(color!(153)),
            ThemeItem::PopupItemHover => Style::color(color!(0), color!(173)),
            ThemeItem::PopupItem => Style::color(color!(0), color!(183)),
            ThemeItem::InfoBarColor => Style::color(color!(0), color!(93)),
            ThemeItem::DirtyBufferMark => Style::color(color!(223), color!(0)),
        };

        Theme::from_hash_map(items)
    };
}
