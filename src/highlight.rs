use std::ops::RangeInclusive;
use std::collections::HashMap;
use ropey::RopeSlice;
use crate::rope::Rope;

pub enum Decoration {
    Normal,
    Keyword,
}

pub struct Span {
    range: RangeInclusive<usize>,
    deco: Decoration
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
    fn priority(&self) -> usize;
    fn highlight(&mut self, lines: RangeInclusive<usize>, rope: &Rope);
    fn provide(&self, line: usize) -> &[Span];
}

/// Cached highlighted text and states.
pub struct HighlightedText {
    /// The highlighted spans merged from `syntax` and `provided`. Each inner
    /// `Vec` represents each line.
    lines: Vec<Vec<Span>>,
    /// Highlighters (e.g. syntax highlighting and LSP clients). Sorted by their
    /// priorities (ones with lower priorities are stored in eariler indices).
    providers: Vec<Box<dyn HighlightProvider>>,
}

impl HighlightedText {
    pub fn new() -> HighlightedText {
        HighlightedText {
            lines: Vec::new(),
            providers: Vec::new(),
        }
    }

    pub fn add_provider(&mut self, provider: Box<dyn HighlightProvider>) {
        self.providers.push(provider);
    }

    /// Invalidates (or clears) highlighted spans from the given line.
    pub fn invalidate(&mut self, line_from: usize) {
        self.lines.truncate(line_from);
    }

    /// Returns highlighted spans at the given line.
    pub fn line_at(&mut self, line: usize, rope: &Rope) -> &[Span] {
        &self.lines[line]
    }

    /// Invokes highlight providers. Note that highlighted spans are not
    /// collected from them until `update` is called.
    pub fn highlight(&mut self, lines: RangeInclusive<usize>, rope: &Rope) {
        if self.lines.len() > *lines.end() {
            // We already have a cache in `self.lines`.
            return;
        }

        let start = self.lines.len();
        for provider in &mut self.providers {
            provider.highlight(start..=*lines.end(), rope);
        }
    }

    /// Collects and merges highlight spans from providers.
    pub fn update(&mut self, lines: std::ops::RangeInclusive<usize>) {
        let start = self.lines.len();
        for i in lines {
            let mut merged = Vec::new();
            for provider in &self.providers {
                let spans = provider.provide(i);
                // TODO: merge into merged
            }

            self.lines[i] = merged;
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
        spans
    }
}

/// Syntax highlighter.
pub struct SyntaxHighlighter {
    lines: Vec<Vec<Span>>,
    states: Vec<SyntaxHighlighterState>,
}

impl SyntaxHighlighter {
    pub fn new() -> SyntaxHighlighter {
        SyntaxHighlighter {
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

    fn highlight(&mut self, lines: std::ops::RangeInclusive<usize>, rope: &Rope) {
        self.lines.truncate(*lines.start());
        self.states.truncate(*lines.start());
        let mut spans = Vec::new();
        for i in lines.clone() {
            let mut state = if i == 0 {
                SyntaxHighlighterState::new()
            } else {
                self.states[i - 1].clone()
            };

            spans.extend(state.highlight_line(rope.line(i)));
            self.states.push(state);
        }
    }

    fn provide(&self, line: usize) -> &[Span] {
        &self.lines[line]
    }
}
