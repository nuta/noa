use crate::highlight::Style;
use crate::language::SpanType;
use crate::status_map::LineStatusType;
use crossterm::style::Color;
use std::collections::HashMap;
use std::io::Stdout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeItem {
    Span(SpanType),
    LineNo,
    LineNoPadding,
    CurrentLineNo,
    LineStatus(LineStatusType),
    LineStatusPadding,
    CommandBoxPrompt,
    ScrollBarVisible,
    PopupItemHover,
    PopupItem,
    InfoBarColor,
    DirtyBufferMark,
    DiagnosticError,
    DiagnosticWarning,
    DiagnosticMessage,
    HoverMessage,
}

pub struct Theme {
    items: HashMap<ThemeItem, Style>,
}

impl Theme {
    pub fn from_hash_map(items: HashMap<ThemeItem, Style>) -> Theme {
        Theme { items }
    }

    pub fn apply(&self, stdout: &mut Stdout, item: ThemeItem) -> crossterm::Result<()> {
        self.items[&item].apply(stdout)
    }

    pub fn apply_span(&self, stdout: &mut Stdout, span: SpanType) -> crossterm::Result<()> {
        self.apply(stdout, ThemeItem::Span(span))
    }
}

macro_rules! color {
    ($n:expr) => {
        Color::AnsiValue($n)
    };
}

// The default theme.
lazy_static! {
    pub static ref THEME: Theme = {
        let items = hashmap! {
            ThemeItem::Span(SpanType::Cursor) => Style::inverted(),
            ThemeItem::Span(SpanType::Selection) => Style::inverted(),
            ThemeItem::Span(SpanType::Normal) => Style::normal(),
            ThemeItem::Span(SpanType::StringLiteral) => Style::fg(Color::DarkYellow),
            ThemeItem::Span(SpanType::EscapedChar) => Style::fg(Color::Magenta),
            ThemeItem::Span(SpanType::Comment) => Style::fg(Color::DarkGreen),
            ThemeItem::Span(SpanType::CtrlKeyword) => Style::fg(Color::Cyan),
            ThemeItem::Span(SpanType::CompilerDirective) => Style::fg(Color::Magenta),
            ThemeItem::LineNo => Style::normal(),
            ThemeItem::CurrentLineNo => Style::new(color!(15), color!(242), true, false, false),
            ThemeItem::LineNoPadding => Style::normal(),
            ThemeItem::LineStatus(LineStatusType::Added) => Style::bg(color!(34)),
            ThemeItem::LineStatus(LineStatusType::Deleted) => Style::bg(color!(198)),
            ThemeItem::LineStatus(LineStatusType::Modified) => Style::bg(color!(33)),
            ThemeItem::LineStatus(LineStatusType::Error) => Style::bg(color!(196)),
            ThemeItem::LineStatus(LineStatusType::Warning) => Style::bg(color!(220)),
            ThemeItem::LineStatusPadding => Style::bg(color!(240)),
            ThemeItem::CommandBoxPrompt => Style::inverted(),
            ThemeItem::ScrollBarVisible => Style::bg(color!(239)),
            ThemeItem::PopupItemHover => Style::fg_bg(color!(15), color!(19)),
            ThemeItem::PopupItem => Style::fg_bg(color!(15), color!(237)),
            ThemeItem::InfoBarColor => Style::inverted(),
            ThemeItem::DirtyBufferMark => Style::fg_bg(color!(238), color!(220)),
            ThemeItem::DiagnosticError => Style::underline(),
            ThemeItem::DiagnosticWarning => Style::underline(),
            ThemeItem::DiagnosticMessage => Style::new(color!(0), color!(229), true, true, false),
            ThemeItem::HoverMessage => Style::new(color!(0), color!(193), true, true, false),
        };

        Theme::from_hash_map(items)
    };
}
