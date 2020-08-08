use std::cmp::min;
use std::ops::RangeInclusive;
use std::collections::HashMap;
use ropey::RopeSlice;
use crossterm::style::{Attribute, Color};
use crate::buffer::Snapshot;
use crate::rope::Rope;
use crate::highlight::{Span, SpanType, Highlighter, HighlightProvider};
use crate::language::Language;

#[derive(Clone)]
enum Context {
    Normal,
    InString(&'static str /* end */, &'static Option<char> /* escape */),
}

#[derive(Clone)]
struct SyntaxHighlighterState {
    context: Context,
    lang: &'static Language,
    spans: Vec<Span>,
    current_line: String,
    current_span_start: usize,
    current_span_len: usize,
}

impl SyntaxHighlighterState {
    pub fn new() -> SyntaxHighlighterState {
        SyntaxHighlighterState {
            context: Context::Normal,
            lang: &crate::language::C,
            spans: Vec::new(),
            current_line: String::new(),
            current_span_start: 0,
            current_span_len: 0,
        }
    }

    pub fn highlight_line<'a>(&mut self, line: RopeSlice<'a>) -> Vec<Span> {
        let mut chars = line.chars().peekable();
        let mut line_str = line.to_string();
        self.reset_spans(line_str);
        'outer: loop {
            match self.context {
                Context::Normal => {
                    'reserved_word_loop: loop {
                        for (opened_by, closed_by, escaped_by) in self.lang.strings {
                            if self.remaining_line().starts_with(opened_by) {
                                self.enter_span(Context::InString(closed_by, escaped_by));
                                self.consume_chars(opened_by.len());
                                break 'reserved_word_loop;
                            }
                        }

                        match chars.next() {
                            Some(ch) => {
                                self.consume_chars(1);
                            }
                            None => {
                                break 'outer;
                            }
                        }
                    }
                }
                Context::InString(closed_by, escape) => {
                    match (chars.next(), chars.peek()) {
                        (Some(ch), Some(_)) if Some(ch) == *escape => {
                            // An escaped character.
                            chars.next(); // consume the escaped char
                            self.consume_chars(2);
                        }
                        (Some(_), _) if self.remaining_line().starts_with(closed_by) => {
                            // Closing the current string literal.
                            self.consume_chars(closed_by.len());
                            self.context = Context::Normal;
                        }
                        (Some(_), _) => {
                            // A character in string.
                            self.consume_chars(1);
                        }
                        (None, _) => {
                            break;
                        }
                    }
                }
            }
        }

        self.spans.clone()
    }

    fn remaining_line(&self) -> &str {
        // XXX: I hope the line is not too long and the cost of this method is
        //      negligible...
        let index = self.current_span_start + self.current_span_len;
        let mut iter = self.current_line.as_str().chars();
        for _ in 0..index {
            iter.next();
        }
        iter.as_str()
    }

    fn reset_spans(&mut self, new_line: String) {
        self.current_line = new_line;
        self.current_span_len = 0;
        self.current_span_start = 0;
        self.spans.clear();
    }

    fn commit_current_span(&mut self) {
        if self.current_span_len > 0 {
            let span_type = match self.context {
                Context::Normal => SpanType::Normal,
                Context::InString(..) => SpanType::StringLiteral,
            };

            let range =
                self.current_span_start..=(self.current_span_start + self.current_span_len - 1);
            self.spans.push(Span::new(span_type, range));
        }
    }

    fn enter_span(&mut self, new_context: Context) {
        self.commit_current_span();
        self.context = new_context;
        self.current_span_start += self.current_span_len;
        self.current_span_len = 0;
    }

    fn consume_chars(&mut self, new_chars: usize) {
        self.current_span_len += new_chars;
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

    fn highlight(
        &mut self,
        lines: std::ops::RangeInclusive<usize>,
        snapshot: &Snapshot
    ) {
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
