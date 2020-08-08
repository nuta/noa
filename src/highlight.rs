use std::cmp::min;
use std::ops::RangeInclusive;
use std::collections::HashMap;
use ropey::RopeSlice;
use crossterm::style::{Attribute, Color};
use regex::Regex;
use crate::buffer::Snapshot;
use crate::rope::Rope;
use crate::language::Language;

// The default theme.
lazy_static! {
    static ref THEME: HashMap<SpanType, Style> = {
        let mut hash = HashMap::new();
        hash.insert(SpanType::Normal, Style::normal());
        hash.insert(SpanType::StringLiteral, Style::fg(Color::Cyan));
        hash.insert(SpanType::EscapedChar, Style::fg(Color::Cyan));
        hash.insert(SpanType::Comment, Style::fg(Color::Cyan));
        hash.insert(SpanType::CtrlKeyword, Style::fg(Color::Cyan));
        hash
    };
}

#[derive(Clone, Debug)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub underline: bool,
    pub inverted: bool,
}

impl Style {
    pub fn new(
        fg: Color,
        bg: Color,
        bold: bool,
        underline: bool,
        inverted: bool
    ) -> Style {
        Style {
            fg,
            bg,
            bold,
            underline,
            inverted,
        }
    }

    pub fn normal() -> Style {
        Style::new(Color::Reset, Color::Reset, false, false, false)
    }

    pub fn fg(color: Color) -> Style {
        Style::new(color, Color::Reset, false, false, false)
    }

    pub fn bold() -> Style {
        Style::new(Color::Reset, Color::Reset, true, false, false)
    }

    pub fn apply(&self, stdout: &mut std::io::Stdout) -> crossterm::Result<()> {
        use crossterm::{queue, style::*};
        use std::io::Write;

        if self.fg != Color::Reset {
            queue!(stdout, SetForegroundColor(self.fg))?;
        }

        if self.bg != Color::Reset {
            queue!(stdout, SetBackgroundColor(self.bg))?;
        }

        if self.bold {
            queue!(stdout, SetAttribute(Attribute::Bold))?;
        }

        if self.underline {
            queue!(stdout, SetAttribute(Attribute::Underlined))?;
        }

        if self.inverted {
            queue!(stdout, SetAttribute(Attribute::Reverse))?;
        }

        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SpanType {
    Normal,
    StringLiteral,
    EscapedChar,
    Comment,
    CtrlKeyword,
}

#[derive(Clone, Debug)]
pub struct Span {
    pub range: RangeInclusive<usize>,
    pub style: &'static Style,
}

impl Span {
    pub fn new(span_type: SpanType, range: RangeInclusive<usize>) -> Span {
        Span {
            style: &THEME[&span_type],
            range,
        }
    }
}

impl PartialEq for Span {
    fn eq(&self, other: &Span) -> bool {
        self.range == other.range
    }
}

impl Eq for Span {}

impl Ord for Span {
    fn cmp(&self, other: &Span) -> std::cmp::Ordering {
        self.range.start().cmp(other.range.start())
    }
}

impl PartialOrd for Span {
    fn partial_cmp(&self, other: &Span) -> Option<std::cmp::Ordering> {
        Some(self.range.start().cmp(other.range.start()))
    }
}

pub trait HighlightProvider {
    fn highlight(
        &mut self,
        snapshot: &Snapshot,
        lines: RangeInclusive<usize>,
    ) -> Vec<Vec<Span>>;
}

/// Cached highlighted text and states.
pub struct Highlighter {
    /// The merged spans. Each inner `Vec` represents each line.
    lines: Vec<Vec<Span>>,
}

impl Highlighter {
    pub fn new() -> Highlighter {
        Highlighter {
            lines: Vec::new(),
        }
    }

    /// Returns highlighted spans at the given line.
    pub fn line(&self, line: usize) -> &[Span] {
        if line >= self.lines.len() {
            return &[];
        }

        &self.lines[line]
    }

    /// Invokes highlight providers and update the highlights.
    pub fn highlight(
        &mut self,
        snapshot: Snapshot,
        lines: RangeInclusive<usize>,
        lang: &'static Language,
    ) {
        self.lines.truncate(*lines.start());
        let end = min(*lines.end(), snapshot.buf.num_lines().saturating_sub(1));
        let range = self.lines.len()..=end;

        // Invoke highlighters.
        for i in range {
            let mut merged: Vec<Span> = Vec::new();

            // Syntax highlighting.
            let merged = highlight_line_by_regexes(
                SpanType::CtrlKeyword,
                &lang.highlights,
                &snapshot.buf.line(i).to_string()
            );

            self.lines.push(merged);
        }
    }
}

fn highlight_line_by_regexes(
    span_type: SpanType,
    regexes: &[Regex],
    line: &str
) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut remaining = line;
    let mut base = 0;
    'outer: loop {
        for regex in regexes {
            if let Some(m) = regex.find(remaining) {
                spans.push(Span::new(span_type, base + m.start()..=(base + m.end() - 1)));
                remaining = &remaining[m.end()..];
                base += m.end();
                continue 'outer;
            }
        }

        break;
    }

    spans
}
