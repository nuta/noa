use std::cmp::min;
use std::ops::RangeInclusive;
use std::collections::HashMap;
use ropey::RopeSlice;
use crossterm::style::{Attribute, Color};
use regex::Regex;
use crate::buffer::Snapshot;
use crate::rope::{Rope, Range, Cursor};
use crate::language::{SpanType, Language};

// The default theme.
lazy_static! {
    pub static ref THEME: HashMap<SpanType, Style> = hashmap! {
        SpanType::Cursor => Style::inverted(),
        SpanType::Selection => Style::inverted(),
        SpanType::Normal => Style::normal(),
        SpanType::StringLiteral => Style::fg(Color::Cyan),
        SpanType::EscapedChar => Style::fg(Color::Cyan),
        SpanType::Comment => Style::fg(Color::Cyan),
        SpanType::CtrlKeyword => Style::fg(Color::Cyan),
    };
}

#[derive(Clone, Debug)]
pub struct Span {
    pub span_type: SpanType,
    pub range: RangeInclusive<usize>,
}

impl Span {
    pub fn new(span_type: SpanType, range: RangeInclusive<usize>) -> Span {
        Span {
            span_type,
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

    pub fn inverted() -> Style {
        Style::new(Color::Reset, Color::Reset, true, false, true)
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
        cursors: &[Cursor],
    ) {
        self.lines.truncate(*lines.start());
        let end = min(*lines.end(), snapshot.buf.num_lines().saturating_sub(1));
        let range = self.lines.len()..=end;

        // Invoke highlighters.
        for i in range {
            let line = snapshot.buf.line(i).to_string();
            let mut spans = Vec::with_capacity(8);
            merge_spans(&mut spans, self.highlight_pattern(&line));
            merge_spans(&mut spans, highlight_cursors(cursors, i, &line));
            self.lines.push(spans);
        }
    }

    fn highlight_pattern(&mut self, line: &str) -> Vec<Span> {
        let mut spans = Vec::new();

        spans
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
                let range = base + m.start()..=(base + m.end() - 1);
                spans.push(Span::new(span_type, range));
                remaining = &remaining[m.end()..];
                base += m.end();
                continue 'outer;
            }
        }

        break;
    }

    spans
}

fn highlight_cursors(cursors: &[Cursor], i: usize, line: &str) -> Vec<Span> {
    let num_chars = line.chars().count();
    let mut spans = Vec::new();
    for (j, cursor) in cursors.iter().enumerate() {
        match cursor {
            Cursor::Normal { pos } if j > 0 && pos.y == i => {
                spans.push(Span::new(SpanType::Cursor, pos.x..=pos.x));
            }
            Cursor::Normal { .. } => {
            }
            Cursor::Selection(range) => {
                use std::cmp::Ordering;
                let front = range.front();
                let back = range.back();
                match (i.cmp(&front.y), i.cmp(&back.y)) {
                    (Ordering::Greater, Ordering::Less) => {
                        spans.push(Span::new(
                            SpanType::Selection,
                            0..=num_chars
                        ));
                    }
                    (Ordering::Equal, Ordering::Less) => {
                        spans.push(Span::new(
                            SpanType::Selection,
                            front.x..=num_chars
                        ));
                    }
                    (Ordering::Greater, Ordering::Equal) => {
                        spans.push(Span::new(
                            SpanType::Selection,
                            0..=back.x.saturating_sub(1)
                        ));
                    }
                    (Ordering::Equal, Ordering::Equal) => {
                        spans.push(Span::new(
                            SpanType::Selection,
                            front.x..=back.x.saturating_sub(1)
                        ));
                    }
                    _ => {
                    // Out of the selection.
                    }
                }
            }
        }
    }

    spans
}

fn merge_spans(spans: &mut Vec<Span>, new_spans: Vec<Span>) {
    // TODO:
    if new_spans.is_empty() {
        return;
    }

    spans.clear();
    for span in new_spans {
        spans.push(span);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn do_highlight(h: &mut Highlighter, text: &str, lines: RangeInclusive<usize>) {
        let mut buf = crate::buffer::Buffer::from_str(text);
        let snapshot = buf.snapshot();
        let lang = &crate::language::PLAIN;
        h.highlight(snapshot, lines, lang, &[Cursor::new(0, 0)]);
    }

    #[test]
    fn highlight_simple_line() {
        let mut h = Highlighter::new();
        do_highlight(&mut h, "", 0..=0);
        assert_eq!(h.line(0), &[]);

        let mut h = Highlighter::new();
        do_highlight(&mut h, "if", 0..=0);
        assert_eq!(h.line(0), &[
            Span::new(SpanType::CtrlKeyword, 0..=1),
        ]);
    }

    #[test]
    fn merge_spans() {
        // TODO:
    }
}
