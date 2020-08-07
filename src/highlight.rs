use std::cmp::min;
use std::ops::RangeInclusive;
use std::collections::HashMap;
use ropey::RopeSlice;
use crossterm::style::{Attribute, Color};
use crate::buffer::Snapshot;
use crate::rope::Rope;

// The default theme.
lazy_static! {
    static ref THEME: HashMap<SpanType, Style> = {
        let mut hash = HashMap::new();
        hash.insert(SpanType::Normal, Style::normal());
        hash.insert(SpanType::TODO, Style::bold());
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
    TODO,
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
    fn name(&self) -> &'static str;
    /// Returns the priority. Lower number is more prioritized (i.e.
    /// overwrites other lower priority stylings).
    fn priority(&self) -> usize;
    fn highlight(&mut self, lines: RangeInclusive<usize>, snapshot: &Snapshot);
    fn provide(&self, line: usize) -> (&[Span], &Snapshot);
}

/// Cached highlighted text and states.
pub struct Highlighter {
    snapshot: Snapshot,
    /// The highlighted spans merged from `syntax` and `provided`. Each inner
    /// `Vec` represents each line.
    lines: Vec<Vec<Span>>,
    /// Highlighters (e.g. syntax highlighting and LSP clients). Sorted by their
    /// priorities (ones with higher priorities are stored in eariler indices).
    providers: Vec<Box<dyn HighlightProvider>>,
}

impl Highlighter {
    pub fn new(snapshot: Snapshot) -> Highlighter {
        Highlighter {
            snapshot,
            lines: Vec::new(),
            providers: Vec::new(),
        }
    }

    pub fn add_provider(&mut self, provider: Box<dyn HighlightProvider>) {
        self.providers.push(provider);
        self.providers.sort_by(|a, b| a.priority().cmp(&b.priority()));
    }

    /// Invalidates (or clears) highlighted spans from the given line.
    pub fn invalidate(&mut self, line_from: usize) {
        self.lines.truncate(line_from);
    }

    /// Returns highlighted spans at the given line.
    pub fn line_at(&mut self, line: usize) -> &[Span] {
        &self.lines[line]
    }

    /// Invokes highlight providers and update the highlights.
    pub fn highlight(
        &mut self,
        lines: RangeInclusive<usize>,
        snapshot: Snapshot
    ) {
        if self.lines.len() > *lines.end() {
            // We already have a cache in `self.lines`.
            return;
        }

        let end = min(*lines.end(), snapshot.buf.num_lines().saturating_sub(1));
        self.snapshot = snapshot;
        let range = self.lines.len()..=end;
        for provider in &mut self.providers {
            provider.highlight(range.clone(), &self.snapshot);
        }

        // Merge highlighted spans.
        for i in range {
            let mut merged: Vec<Span> = Vec::new();
            for provider in &self.providers {
                let (spans, snapshot) = provider.provide(i);
                if *snapshot != self.snapshot {
                    continue;
                }

                for span in spans {
                    let overlaps = merged.iter().any(|new_span| {
                        new_span.range.contains(span.range.start())
                            || new_span.range.contains(span.range.end())
                    });

                    if !overlaps {
                        merged.push(span.clone());
                    }
                }
            }

            self.lines.push(merged);
        }
    }
}

#[derive(Clone)]
struct SyntaxHighlighterState {
}

impl SyntaxHighlighterState {
    pub fn new() -> SyntaxHighlighterState {
        SyntaxHighlighterState {
        }
    }

    pub fn highlight_line<'a>(&mut self, line: RopeSlice<'a>) -> Vec<Span> {
        let mut spans = Vec::new();
        spans.push(Span::new(SpanType::TODO,
            0..=std::cmp::min(line.len_chars().saturating_sub(1), 3),
            ));
        spans
    }
}

/// Syntax highlighter.
pub struct SyntaxHighlighter {
    snapshot: Option<Snapshot>,
    lines: Vec<Vec<Span>>,
    states: Vec<SyntaxHighlighterState>,
}

impl SyntaxHighlighter {
    pub fn new() -> SyntaxHighlighter {
        SyntaxHighlighter {
            snapshot: None,
            lines: Vec::new(),
            states: Vec::new(),
        }
    }
}

impl HighlightProvider for SyntaxHighlighter {
    fn name(&self) -> &'static str {
        "syntax"
    }

    fn priority(&self) -> usize {
        100
    }

    fn highlight(&mut self, lines: std::ops::RangeInclusive<usize>, snapshot: &Snapshot) {
        self.snapshot = Some(snapshot.clone());
        self.lines.truncate(*lines.start());
        self.states.truncate(*lines.start());
        for i in lines {
            let mut state = if i == 0 {
                SyntaxHighlighterState::new()
            } else {
                self.states[i - 1].clone()
            };

            self.lines.push(state.highlight_line(snapshot.buf.line(i)));
            self.states.push(state);
        }
    }

    fn provide(&self, line: usize) -> (&[Span], &Snapshot) {
        (&self.lines[line], self.snapshot.as_ref().unwrap())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::buffer::Buffer;

    #[test]
    fn highlight_empty_buffer() {
        let mut buffer = Buffer::new();
        let mut highlighter = Highlighter::new(buffer.snapshot());
        highlighter.add_provider(Box::new(SyntaxHighlighter::new()));
        highlighter.highlight(0..=0, buffer.snapshot());
    }

    #[test]
    fn highlight_simple_c_function() {
        let mut buffer = Buffer::new();
        buffer.insert("int min(int a, int b) {\n");
        buffer.insert("    return (a < b) ? a : b;\n");
        buffer.insert("}\n");

        let mut highlighter = Highlighter::new(buffer.snapshot());
        highlighter.add_provider(Box::new(SyntaxHighlighter::new()));

        highlighter.highlight(0..=2, buffer.snapshot());
        highlighter.line_at(0);
        highlighter.line_at(1);
        highlighter.line_at(2);
    }

    #[test]
    fn invalidation() {
        let mut buffer = Buffer::new();
        buffer.insert("int min(int a, int b) {\n");
        buffer.insert("    return (a < b) ? a : b;\n");
        buffer.insert("}\n");

        let mut highlighter = Highlighter::new(buffer.snapshot());
        highlighter.add_provider(Box::new(SyntaxHighlighter::new()));

        highlighter.highlight(0..=2, buffer.snapshot());
        highlighter.line_at(0);
        highlighter.invalidate(1);
        highlighter.highlight(1..=2, buffer.snapshot());
        highlighter.line_at(1);
        highlighter.line_at(2);
    }
}
