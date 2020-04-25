use crate::language::Language;

#[derive(Debug)]
enum Context {

}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Normal,
    CtrlStmt,
    LineComment,
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
        while i < line.len() {
            eprintln!("rest = '{}', context = {:?}", &line[i..], self.context);
            let mut matched = None;
            for (style, keywords) in keyword_types {
                if let Some(k) = consume(keywords, &line[i..]) {
                    matched = Some((*style, k.len()));
                    break;
                }
            }

            if matched.is_none() {
                if let Some(comment_start) = consume(&self.lang.line_comments, &line[i..]) {
                    matched = Some((Style::LineComment, line.len() - i));
                }
            }

            let n = if let Some((style, n)) = matched {
                if i != normal_start {
                    spans.push((style, &line[normal_start..i]));
                    normal_start = i;
                }

                spans.push((style, &line[i..i + n]));
                normal_start += n;
                n
            } else {
                let mut n = 1;
                let mut iter = line.chars().skip(i);
                if is_word_char(iter.next().unwrap()) {
                    while let Some(ch) = iter.next() {
                        if !is_word_char(ch) {
                            break;
                        }

                        n += 1;
                    }
                }
                n
            };

            i += n;
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
        assert_eq!(h.highlight_line("barif-if (foo) // fii"), vec![]);
    }
}
