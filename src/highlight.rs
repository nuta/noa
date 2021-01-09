use crate::buffer::Snapshot;
use crate::language::{Language, Pattern, SpanType};
use crate::rope::Cursor;
use crossterm::style::Color;
use regex::Regex;
use std::cmp::{max, min};
use std::ops::RangeInclusive;

#[derive(Clone, Debug)]
pub struct Span {
    pub span_type: SpanType,
    pub range: RangeInclusive<usize>,
}

impl Span {
    pub fn new(span_type: SpanType, range: RangeInclusive<usize>) -> Span {
        Span { span_type, range }
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
    pub fn new(fg: Color, bg: Color, bold: bool, underline: bool, inverted: bool) -> Style {
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

    pub fn bg(color: Color) -> Style {
        Style::new(Color::Reset, color, false, false, false)
    }

    // pub fn color(fg: Color, bg: Color) -> Style {
    //     Style::new(fg, bg, false, false, false)
    // }

    #[allow(unused)]
    pub fn bold() -> Style {
        Style::new(Color::Reset, Color::Reset, true, false, false)
    }

    pub fn inverted() -> Style {
        Style::new(Color::Reset, Color::Reset, true, false, true)
    }

    pub fn underline() -> Style {
        Style::new(Color::Reset, Color::Reset, false, true, false)
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
    fn highlight(&mut self, snapshot: &Snapshot, lines: RangeInclusive<usize>) -> Vec<Vec<Span>>;
}

/// Cached highlighted text and states.
pub struct Highlighter {
    /// The merged spans. Each inner `Vec` represents each line.
    lines: Vec<Vec<Span>>,
    lang: &'static Language,
    patterns_stack: Vec<(
        &'static [&'static str],
        Option<(SpanType, &'static Regex, &'static [SpanType])>,
    )>,
}

impl Highlighter {
    pub fn new(lang: &'static Language) -> Highlighter {
        Highlighter {
            lines: Vec::new(),
            lang,
            patterns_stack: Vec::new(),
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
        cursors: &[Cursor],
    ) {
        self.patterns_stack = vec![(self.lang.top_level_patterns, None)];
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
        let mut index = 0;
        'outer: loop {
            let current = self.patterns_stack.last().unwrap();
            let remaining = &line[index..];
            let current_inner = current.1.map(|(inner, _, _)| inner);

            for pattern_name in current.0 {
                match &self.lang.patterns[pattern_name] {
                    Pattern::Inline { regex, captures } => {
                        if let Some(groups) = regex.captures(remaining) {
                            add_captured_spans(
                                &mut spans,
                                &mut index,
                                captures,
                                &groups,
                                current_inner,
                            );
                            continue 'outer;
                        }
                    }
                    Pattern::Block {
                        start,
                        end,
                        start_captures,
                        end_captures,
                        inner,
                        patterns,
                    } => {
                        if let Some(groups) = start.captures(remaining) {
                            add_captured_spans(
                                &mut spans,
                                &mut index,
                                start_captures,
                                &groups,
                                current_inner,
                            );

                            self.patterns_stack
                                .push((patterns, Some((*inner, end, end_captures))));
                            continue 'outer;
                        }
                    }
                }
            }

            if let Some((inner, end, end_captures)) = current.1 {
                // Try end of the current block.
                if let Some(groups) = end.captures(remaining) {
                    add_captured_spans(&mut spans, &mut index, end_captures, &groups, Some(inner));

                    // Leave from the current block.
                    self.patterns_stack.pop();
                    continue 'outer;
                } else if !remaining.is_empty() {
                    // In the current block inner contents.
                    let range = index..=(index + remaining.chars().count() - 1);
                    spans.push(Span::new(inner, range));
                }
            }

            break;
        }

        spans
    }
}

fn add_captured_spans(
    spans: &mut Vec<Span>,
    index: &mut usize,
    captures: &[SpanType],
    groups: &regex::Captures,
    mut inner: Option<SpanType>,
) {
    let mut index_diff = 0;
    // Skip the first group that matches the whole regex, not each group
    // enclosed by `(..)`.
    for (i, m) in groups.iter().skip(1).enumerate() {
        if let Some(m) = m {
            if let Some(span_type) = &inner {
                if m.start() > 0 {
                    let range = *index..=(*index + m.start() - 1);
                    spans.push(Span::new(*span_type, range));
                }

                // Use .take() to do this operation only once.
                inner.take();
            }

            let span_type = captures[i];
            let range = *index + m.start()..=(*index + m.end() - 1);
            spans.push(Span::new(span_type, range));
            index_diff = m.end();
        }
    }

    assert!(index_diff > 0);
    *index += index_diff;
}

fn highlight_cursors(cursors: &[Cursor], i: usize, line: &str) -> Vec<Span> {
    let num_chars = line.chars().count();
    let mut spans = Vec::new();
    for (j, cursor) in cursors.iter().enumerate() {
        match cursor {
            Cursor::Normal { pos } if j > 0 && pos.y == i => {
                spans.push(Span::new(SpanType::Cursor, pos.x..=pos.x));
            }
            Cursor::Normal { .. } => {}
            Cursor::Selection(range) => {
                use std::cmp::Ordering;
                let front = range.front();
                let back = range.back();
                match (i.cmp(&front.y), i.cmp(&back.y)) {
                    (Ordering::Greater, Ordering::Less) => {
                        spans.push(Span::new(SpanType::Selection, 0..=num_chars));
                    }
                    (Ordering::Equal, Ordering::Less) => {
                        spans.push(Span::new(SpanType::Selection, front.x..=num_chars));
                    }
                    (Ordering::Greater, Ordering::Equal) => {
                        spans.push(Span::new(SpanType::Selection, 0..=back.x.saturating_sub(1)));
                    }
                    (Ordering::Equal, Ordering::Equal) => {
                        spans.push(Span::new(
                            SpanType::Selection,
                            front.x..=back.x.saturating_sub(1),
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

fn overlaps(a: &RangeInclusive<usize>, b: &RangeInclusive<usize>) -> bool {
    a.start() <= b.end() && b.start() <= a.end()
}

fn preferred_span(a: SpanType, b: SpanType) -> SpanType {
    match (a, b) {
        (SpanType::Selection, _) | (_, SpanType::Selection) => SpanType::Selection,
        (SpanType::Cursor, _) | (_, SpanType::Cursor) => SpanType::Cursor,
        (SpanType::Comment, _) | (_, SpanType::Comment) => SpanType::Comment,
        (_, _) => a,
    }
}

/// Assumes `spans` is already sorted by its ranges.
fn merge_spans(spans: &mut Vec<Span>, spans2: Vec<Span>) {
    spans.extend(spans2);

    loop {
        let mut split_at = None;
        for i in 0..spans.len() {
            for j in 0..spans.len() {
                if i < j && overlaps(&spans[i].range, &spans[j].range) {
                    split_at = Some((i, j));
                    break;
                }
            }
        }

        if let Some((i, j)) = split_at {
            //
            //
            //  |--- a ---|
            //         |--- b --|
            //  |xxxxxxyyyyzzzzzz
            //  ^      ^  ^     ^
            // t0     t1  t2    t3
            let a = &spans[i];
            let b = &spans[j];
            let t0 = min(*a.range.start(), *b.range.start());
            let t1 = max(*a.range.start(), *b.range.start());
            let t2 = min(*a.range.end(), *b.range.end());
            let t3 = max(*a.range.end(), *b.range.end());
            let y = preferred_span(a.span_type, b.span_type);
            let (x, z) = if t0 == *a.range.start() && t3 == *a.range.end() {
                (a.span_type, a.span_type)
            } else if t0 == *b.range.start() && t3 == *b.range.end() {
                (b.span_type, b.span_type)
            } else if a.range.start() < b.range.start() {
                (a.span_type, b.span_type)
            } else {
                (b.span_type, a.span_type)
            };

            spans.remove(j);
            spans.remove(i);
            if t0 < t1 {
                spans.push(Span::new(x, t0..=(t1 - 1)));
            }
            spans.push(Span::new(y, t1..=t2));
            if t2 < t3 {
                spans.push(Span::new(z, (t2 + 1)..=t3));
            }
        } else {
            break;
        }
    }

    spans.sort_by(|a, b| a.range.start().cmp(b.range.start()));

    // Merge continuous spans with the same span type.
    let mut i = 1;
    while i < spans.len() {
        if spans[i - 1].span_type == spans[i].span_type
            && *spans[i - 1].range.end() + 1 == *spans[i].range.start()
        {
            spans[i - 1].range = *spans[i - 1].range.start()..=*spans[i].range.end();
            spans.remove(i);
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn do_highlight(h: &mut Highlighter, text: &str, lines: RangeInclusive<usize>) {
        let buf = crate::buffer::Buffer::from_str(text);
        let snapshot = buf.snapshot();
        h.highlight(snapshot, lines, &[Cursor::new(0, 0)]);
    }

    #[test]
    fn highlight_simple_inlines() {
        let mut h = Highlighter::new(&crate::language::PLAIN);
        do_highlight(&mut h, "", 0..=0);
        assert_eq!(h.line(0), &[]);

        let mut h = Highlighter::new(&crate::language::PLAIN);
        do_highlight(&mut h, "if", 0..=0);
        assert_eq!(h.line(0), &[Span::new(SpanType::CtrlKeyword, 0..=1),]);

        let mut h = Highlighter::new(&crate::language::PLAIN);
        do_highlight(&mut h, "if for", 0..=0);
        assert_eq!(
            h.line(0),
            &[
                Span::new(SpanType::CtrlKeyword, 0..=1),
                Span::new(SpanType::CtrlKeyword, 3..=5),
            ]
        );
    }

    #[test]
    fn highlight_simple_blocks() {
        // A string literal.
        let mut h = Highlighter::new(&crate::language::PLAIN);
        do_highlight(&mut h, "\"abc\"", 0..=0);
        assert_eq!(h.line(0), &[Span::new(SpanType::StringLiteral, 0..=4),]);

        // Escaped chars
        let mut h = Highlighter::new(&crate::language::PLAIN);
        do_highlight(&mut h, "\"a\\nb\"" /* "a\nb" */, 0..=0);
        assert_eq!(
            h.line(0),
            &[
                Span::new(SpanType::StringLiteral, 0..=1), // "a
                Span::new(SpanType::EscapedChar, 2..=3),   // \n
                Span::new(SpanType::StringLiteral, 4..=5), // b"
            ]
        );

        // Escaped chars.
        let mut h = Highlighter::new(&crate::language::PLAIN);
        do_highlight(&mut h, "\"ab\\\"cd\"" /* "ab\"cd" */, 0..=0);
        assert_eq!(
            h.line(0),
            &[
                Span::new(SpanType::StringLiteral, 0..=2), // "ab
                Span::new(SpanType::EscapedChar, 3..=4),   // \"
                Span::new(SpanType::StringLiteral, 5..=7), // cd"
            ]
        );
    }

    #[test]
    fn test_block_comments() {
        let mut h = Highlighter::new(&crate::language::PLAIN);
        do_highlight(&mut h, "/* if */", 0..=0);
        assert_eq!(h.line(0), &[Span::new(SpanType::Comment, 0..=7)]);
    }

    #[test]
    fn test_preferred_span() {
        assert_eq!(
            preferred_span(SpanType::Selection, SpanType::Comment),
            SpanType::Selection
        );
        assert_eq!(
            preferred_span(SpanType::Comment, SpanType::Selection),
            SpanType::Selection
        );
    }
    #[test]
    fn test_merge_spans() {
        {
            let mut spans = vec![];
            let others = vec![];
            merge_spans(&mut spans, others);
            assert_eq!(spans, vec![]);
        }

        {
            let mut spans = vec![Span::new(SpanType::StringLiteral, 0..=4)];
            let others = vec![];
            merge_spans(&mut spans, others);
            assert_eq!(spans, vec![Span::new(SpanType::StringLiteral, 0..=4)]);
        }

        {
            let mut spans = vec![];
            let others = vec![Span::new(SpanType::StringLiteral, 0..=4)];
            merge_spans(&mut spans, others);
            assert_eq!(spans, vec![Span::new(SpanType::StringLiteral, 0..=4)]);
        }

        // No overlapping.
        {
            let mut spans = vec![Span::new(SpanType::StringLiteral, 0..=2)];
            let others = vec![Span::new(SpanType::CtrlKeyword, 3..=4)];
            merge_spans(&mut spans, others);
            assert_eq!(
                spans,
                vec![
                    Span::new(SpanType::StringLiteral, 0..=2),
                    Span::new(SpanType::CtrlKeyword, 3..=4),
                ]
            );
        }

        // Selection precedes other types.
        {
            let mut spans = vec![Span::new(SpanType::CtrlKeyword, 1..=3)];
            let others = vec![
                Span::new(SpanType::CtrlKeyword, 2..=3),
                Span::new(SpanType::Selection, 0..=7),
            ];
            merge_spans(&mut spans, others);
            assert_eq!(spans, vec![Span::new(SpanType::Selection, 0..=7),]);
        }

        // Selection precedes other types.
        {
            let mut spans = vec![Span::new(SpanType::Selection, 1..=2)];
            let others = vec![Span::new(SpanType::CtrlKeyword, 0..=3)];
            merge_spans(&mut spans, others);
            assert_eq!(
                spans,
                vec![
                    Span::new(SpanType::CtrlKeyword, 0..=0),
                    Span::new(SpanType::Selection, 1..=2),
                    Span::new(SpanType::CtrlKeyword, 3..=3),
                ]
            );
        }

        // Selection precedes other types (should be the same result).
        {
            let mut spans = vec![Span::new(SpanType::CtrlKeyword, 0..=3)];
            let others = vec![Span::new(SpanType::Selection, 1..=2)];
            merge_spans(&mut spans, others);
            assert_eq!(
                spans,
                vec![
                    Span::new(SpanType::CtrlKeyword, 0..=0),
                    Span::new(SpanType::Selection, 1..=2),
                    Span::new(SpanType::CtrlKeyword, 3..=3),
                ]
            );
        }
    }
}
