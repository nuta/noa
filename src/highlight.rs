use crate::language::Language;

#[derive(Debug)]
enum Context {
    Normal,
    InString(usize /* start index */, &'static str /* end */, Option<&'static str> /* escape */),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Normal,
    Ctrl,
    Def,
    LineComment,
    InString,
}

pub struct Highlight {
    lang: &'static Language,
    context: Vec<Context>,
}

fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || "_".contains(ch)
}

fn consume<'a, 'b>(keywords: &'a [&'a str], rest: &'b str) -> Option<&'a str> {
    let mut iter = keywords.iter();
    loop {
        let k = match iter.next() {
            Some(k) => k,
            None => break None,
        };

        if !rest.starts_with(k) {
            continue;
        }

        let next_ch = rest.chars().skip(k.len()).next();
        match next_ch {
            Some(ch) => {
                if !is_word_char(ch) {
                    break Some(k);
                } else {
                    continue;
                }
            }
            None => {
                break Some(k);
            }
        }
    }
}

impl Highlight {
    pub fn new(lang: &'static Language) -> Highlight {
        Highlight {
            lang,
            context: Vec::new(),
        }
    }

    pub fn highlight_line<'a>(&mut self, line: &'a str) -> Vec<(Style, &'a str)> {
        let keyword_types = &[
            (Style::Ctrl, self.lang.keywords.ctrls),
            (Style::Def, self.lang.keywords.defs),
        ];
        let mut spans: Vec<(Style, &str)> = Vec::new();
        let mut normal_start = 0;
        let mut i = 0;
        'outer: while i < line.len() {
            match self.context.last() {
                Some(Context::InString(str_start, end, escape)) => {
                    let str_start = *str_start;
                    let start = i;
                    let mut iter = (&line[i..]).char_indices();
                    let terminated = loop {
                        let next = iter.next();
                        let offset = match next {
                            Some((offset, _)) => offset,
                            None => break false,
                        };

                        // Escaping `end`.
                        if let Some(escape) = escape {
                            if (&line[start + offset..]).starts_with(escape) {
                                iter.nth(escape.len() - 1);
                            }
                        }

                        if (&line[start + offset..]).starts_with(end) {
                            i += offset + end.len();
                            break true;
                        }
                    };

                    // The string is not terminated in the current line.
                    let s = if terminated {
                        self.context.pop();
                        &line[str_start..i]
                    } else {
                        i = line.len();
                        &line[str_start..]
                    };

                    if let Some(span) = spans.last_mut() {
                        span.1 = s;
                    } else {
                        spans.push((Style::InString, s));
                    }

                    normal_start = i;
                }
                Some(Context::Normal) | None => {
                    // Strings.
                    for (start, end, escape) in self.lang.strings {
                        if (&line[i..]).starts_with(start) {
                            if i != normal_start {
                                spans.push((Style::Normal, &line[normal_start..i]));
                            }

                            spans.push((Style::InString, &line[i..i + start.len()]));
                            self.context.push(Context::InString(i, end, *escape));
                            i += start.len();
                            normal_start = i;
                            continue 'outer;
                        }
                    }

                    let mut matched = None;

                    // Keywords.
                    for (style, keywords) in keyword_types {
                        if let Some(k) = consume(keywords, &line[i..]) {
                            matched = Some((*style, k.len()));
                            break;
                        }
                    }

                    // Line comments.
                    if matched.is_none() {
                        if consume(&self.lang.line_comments, &line[i..]).is_some() {
                            matched = Some((Style::LineComment, line.len() - i));
                        }
                    }

                    let n = if let Some((style, n)) = matched {
                        // Consume characters before the matched keyword, etc.
                        if i != normal_start {
                            spans.push((Style::Normal, &line[normal_start..i]));
                            normal_start = i;
                        }

                        spans.push((style, &line[i..i + n]));
                        normal_start += n;
                        n
                    } else {
                        // Skip normal characters.
                        let mut iter = line[i..].char_indices();
                        let ch = iter.next().unwrap().1;
                        let mut n = ch.len_utf8();
                        if is_word_char(ch) {
                            while let Some((offset, ch)) = iter.next() {
                                if !is_word_char(ch) {
                                    break;
                                }

                                n = offset;
                            }
                        }
                        n
                    };

                    i += n;
                }
            }
        }

        if normal_start != i {
            spans.push((Style::Normal, &line[normal_start..]));
        }

        match self.context.last_mut() {
            Some(Context::InString(start, _, _)) => { *start = 0; }
            Some(Context::Normal) | None => {}
        }

        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Style::*;

    #[test]
    fn test_highlight() {
        let lang = &crate::language::CXX;

        assert_eq!(Highlight::new(lang).highlight_line(""), vec![]);
        assert_eq!(Highlight::new(lang).highlight_line("foo"), vec![(Normal, "foo")]);
        assert_eq!(Highlight::new(lang).highlight_line("あいうえお"), vec![(Normal, "あいうえお")]);
        assert_eq!(
            Highlight::new(lang).highlight_line("if (true) { bar(); }"),
            vec![
                (Ctrl, "if"),
                (Normal, " (true) { bar(); }"),
            ]
        );
        assert_eq!(Highlight::new(lang).highlight_line("\""), vec![(InString, "\"")]);
        assert_eq!(
            Highlight::new(lang).highlight_line("puts(\"こんにちは世界\"); break;"),
            vec![
                (Normal, "puts("),
                (InString, "\"こんにちは世界\""),
                (Normal, "); "),
                (Ctrl, "break"),
                (Normal, ";")
            ]
        );
    }

    #[test]
    fn test_string_highlighting() {
        let lang = &crate::language::CXX;

        assert_eq!(
            Highlight::new(lang).highlight_line("abc // if"),
            vec![(Normal, "abc "), (LineComment, "// if")]
        );
        assert_eq!(
            Highlight::new(lang).highlight_line("\"Hello \\\" World\""),
            vec![(InString, "\"Hello \\\" World\"")]
        );

        let mut multiline = Highlight::new(lang);
        assert_eq!(
            multiline.highlight_line("abc\""),
            vec![(Normal, "abc"), (InString, "\"")]
        );
        assert_eq!(
            multiline.highlight_line("x"),
            vec![(InString, "x")]
        );
    }
}
