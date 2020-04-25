use crate::language::Language;

#[derive(Debug)]
enum Context {
    Normal,
    String(usize /* start index */, &'static str /* end */, Option<&'static str> /* escape */),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Normal,
    CtrlStmt,
    LineComment,
    String,
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
            (Style::CtrlStmt, self.lang.ctrl_stmts),
        ];

        let mut spans = Vec::new();
        let mut normal_start = 0;
        let mut i = 0;
        'outer: while i < line.len() {
            eprintln!(">>> rest = '{}', context = {:?}", &line[i..], self.context);
            match self.context.last() {
                Some(Context::String(str_start, end, escape)) => {
                    let str_start = *str_start;
                    let start = i;
                    let mut iter = (&line[i..]).char_indices();
                    let terminated = loop {
                        let next = iter.next();
                        let offset = match next {
                            Some((offset, _)) => offset,
                            None => break false,
                        };

                        eprintln!("string_end = '{}'", &line[start + offset..]);
                        // Escaping `end`.
                        if let Some(escape) = escape {
                            if (&line[start + offset..]).starts_with(escape) {
                                eprintln!("DO escape = '{}'", escape);
                                iter.nth(escape.len() - 1);
                            }
                        }

                        if (&line[start + offset..]).starts_with(end) {
                            i += offset + end.len();
                            break true;
                        }
                    };

                    // The string is not terminated in the current line.
                    if terminated {
                        self.context.pop();
                        spans.push((Style::String, &line[str_start..i]));
                    } else {
                        spans.push((Style::String, &line[str_start..]));
                        i = line.len();
                    }

                    normal_start = i;
                }
                Some(Context::Normal) | None => {
                    // Strings.
                    for (start, end, escape) in self.lang.strings {
                        if (&line[i..]).starts_with(start) {
                            eprintln!("entering string...");
                            if i != normal_start {
                                spans.push((Style::Normal, &line[normal_start..i]));
                            }

                            self.context.push(Context::String(i, end, *escape));
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
                        let mut n = 1;
                        let mut iter = line.chars().skip(i);
                        if is_word_char(iter.next().unwrap()) {
                            while let Some(ch) = iter.next() {
                                if !is_word_char(ch) {
                                    break;
                                }

                                n += 1; // FIXME: multi byte chars
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

        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight() {
        let mut h = Highlight::new(&crate::language::C);
        assert_eq!(h.highlight_line(""), vec![]);
        assert_eq!(h.highlight_line("foo"), vec![(Style::Normal, "foo")]);
        assert_eq!(
            h.highlight_line("\"Hello \\\" World\""),
            vec![(Style::String, "\"Hello \\\" World\"")]
        );

        assert_eq!(
            h.highlight_line("if (true) { bar(); }"),
            vec![
                (Style::CtrlStmt, "if"),
                (Style::Normal, " (true) { bar(); }"),
            ]
        );
    }
}
