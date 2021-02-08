use crate::rope::{Cursor, Point, Range, Rope};
use regex::RegexBuilder;
use std::io::Write;
use std::iter::Peekable;
use std::process::{Command, Stdio};
use std::str::Chars;

#[derive(Debug, Clone)]
pub struct Captures {
    /// The capture can be `None` if the group does not appear in the match.
    pub unnamed: Vec<Option<Range>>,
}

#[derive(Debug, Clone)]
pub struct Match {
    pub range: Range,
    pub captures: Option<Captures>,
}

fn char_index_to_range(rope: &Rope, range: &Range, start: usize, end: usize) -> Range {
    let slice = rope.sub_str(range);
    let start_y = range.start.y + slice.byte_to_line(start);
    let start_x = slice.byte_to_char(start - slice.line_to_byte(start_y - range.start.y));
    let end_y = range.start.y + slice.byte_to_line(end);
    let end_x = slice.byte_to_char(end - slice.line_to_byte(end_y - range.start.y));
    Range::new(start_y, start_x, end_y, end_x)
}

fn process_captures(
    rope: &Rope,
    range: &Range,
    captures: regex::Captures<'_>,
) -> (Range, Captures) {
    let whole_match = captures.get(0).unwrap();
    let whole_range = char_index_to_range(rope, range, whole_match.start(), whole_match.end());

    let mut unnamed = Vec::new();
    for c in captures.iter().skip(1) {
        unnamed.push(c.map(|c| char_index_to_range(rope, range, c.start(), c.end())));
    }

    (whole_range, Captures { unnamed })
}

/// A wrapper of Regex to implement PartialEq to simplify unit tests.
#[derive(Debug)]
struct Regex(regex::Regex);

impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, regex::Error> {
        Ok(Regex(regex::Regex::new(pattern)?))
    }

    pub fn match_all(
        &self,
        rope: &Rope,
        range: &Range,
        max_matches: &Option<usize>,
    ) -> Vec<(Range, Captures)> {
        let heystack = rope.sub_str(range).to_string();
        let iter = self.0.captures_iter(&heystack);
        if let Some(max_matches) = max_matches {
            iter.take(*max_matches)
                .map(|captures| process_captures(rope, range, captures))
                .collect()
        } else {
            iter.map(|captures| process_captures(rope, range, captures))
                .collect()
        }
    }

    pub fn match_first(&self, rope: &Rope, range: &Range) -> Option<(Range, Captures)> {
        let heystack = rope.sub_str(range).to_string();
        self.0
            .captures(&heystack)
            .map(|captures| process_captures(rope, range, captures))
    }
}

impl Into<Regex> for regex::Regex {
    fn into(self) -> Regex {
        Regex(self)
    }
}

impl PartialEq for Regex {
    fn eq(&self, other: &Regex) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

#[derive(Debug, PartialEq)]
enum Address {
    /// `.`
    Current,
    /// `0`, `123`, ...
    LineNo(usize),
    /// `/.../`
    Match(Regex),
    /// `$`
    EOF,
    /// `a1,a2`
    Range {
        start: Box<Address>,
        end: Box<Address>,
    },
    /// `a1+a2`
    Forward {
        from: Box<Address>,
        addr: Box<Address>,
    },
    /// `a1+a2`
    Backward {
        from: Box<Address>,
        addr: Box<Address>,
    },
}

#[derive(Debug, PartialEq)]
enum JumpTo {
    /// (empty)
    FirstMatch,
    /// `^`
    Beginning,
    /// `|`
    AfterIndent,
    /// `$`
    End,
}

#[derive(Debug, PartialEq)]
enum Op {
    /// `g`
    Filter(Regex),
    /// `v`
    FilterOut(Regex),
    /// `x`
    Extract(Regex),
    /// `y`
    ExtractReverse(Regex),
    /// `s`
    SurroundWithOutDelim(Regex),
    /// `S`
    SurroundWithDelim(Regex),
    /// `i`
    Prepend(String),
    /// `a`
    Append(String),
    /// `c`
    ReplaceWith(String),
    /// `d`
    Delete,
    /// `j`
    Jump(JumpTo),
    /// `p`
    ShellCommand(String),
}

#[derive(Debug, PartialEq)]
struct Query {
    addr: Address,
    op: Op,
}

#[derive(Debug, PartialEq)]
pub enum ParseErrorKind {
    EmptyRegex,
    InvalidRegex(regex::Error),
    UnknownOpcode { opcode: char },
}

#[derive(Debug, PartialEq)]
pub struct ParseError {
    cursor: usize,
    kind: ParseErrorKind,
}

#[derive(Debug)]
pub enum ExecutionError {
    CommandError(std::io::Error),
    ExitedWith { stderr: String },
}

#[derive(Debug)]
pub struct Changes {
    pub last_matches: Vec<Match>,
}

pub struct Engine {
    queries: Vec<Query>,
    cursor: Range,
}

fn rope_whole_range(rope: &Rope) -> Range {
    Range::from_points(
        Point::new(0, 0),
        Point::new(rope.num_lines() - 1, rope.line_len(rope.num_lines() - 1)),
    )
}

fn rope_line_range(rope: &Rope, y: usize) -> Option<Range> {
    if y > rope.num_lines() {
        None
    } else {
        let end_x = if y == rope.num_lines() {
            0
        } else {
            rope.line_len(y)
        };

        Some(Range::new(y, 0, y, end_x))
    }
}

impl Engine {
    pub fn new(query: &str) -> Result<Engine, ParseError> {
        let mut parser = Parser::new(query);
        Ok(Engine {
            queries: parser.parse()?,
            cursor: Range::new(0, 0, 0, 0),
        })
    }

    pub fn execute(
        &mut self,
        rope: &mut Rope,
        current: &Range,
        max_matches: Option<usize>,
    ) -> Result<Changes, ExecutionError> {
        let mut matches = vec![Match {
            range: current.clone(),
            captures: None,
        }];

        for query in &self.queries {
            let mut new_matches = Vec::new();
            for m in matches {
                if let Some(m) = self.evaluate_addr(rope, &m.range, &query.addr)? {
                    self.evaluate_op(rope, &mut new_matches, &m, &query.op, &max_matches)?;
                }

                if let Some(max_matches) = max_matches {
                    if new_matches.len() >= max_matches {
                        break;
                    }
                }
            }

            new_matches.sort_by(|a, b| a.range.back().cmp(b.range.back()));
            new_matches.reverse();
            matches = new_matches;
        }

        matches.sort_by(|a, b| a.range.front().cmp(b.range.front()));
        Ok(Changes {
            last_matches: matches,
        })
    }

    fn evaluate_addr(
        &self,
        rope: &Rope,
        current: &Range,
        addr: &Address,
    ) -> Result<Option<Match>, ExecutionError> {
        Ok(match addr {
            Address::Current => Some(Match {
                range: current.clone(),
                captures: None,
            }),
            Address::LineNo(y) if *y == 0 => Some(Match {
                range: Range::new(0, 0, 0, 0),
                captures: None,
            }),
            Address::LineNo(y) => rope_line_range(rope, y - 1).map(|range| Match {
                range,
                captures: None,
            }),
            Address::Match(regex) => {
                let range = Range::from_points(*current.back(), *rope_whole_range(rope).back());
                regex
                    .match_first(&rope, &range)
                    .map(|(range, captures)| Match {
                        range,
                        captures: Some(captures),
                    })
            }
            Address::EOF => Some(Match {
                range: rope_whole_range(rope),
                captures: None,
            }),
            Address::Range { start, end } => {
                let a = self.evaluate_addr(rope, current, start)?;
                let b = self.evaluate_addr(rope, current, end)?;
                match (a, b) {
                    (Some(a), Some(b)) => Some(Match {
                        range: Range::from_points(*a.range.front(), *b.range.back()),
                        captures: None,
                    }),
                    _ => None,
                }
            }
            Address::Forward { from, addr } => {
                let start = self
                    .evaluate_addr(rope, current, from)?
                    .map(|m| m.range.back().clone())
                    .unwrap_or_else(|| current.back().clone());
                unimplemented!()
                // self.evaluate_addr(buffer, current, addr, Some(start))
            }
            Address::Backward { from, addr } => {
                unimplemented!();
            }
        })
    }

    fn evaluate_op(
        &self,
        rope: &mut Rope,
        new_matches: &mut Vec<Match>,
        m: &Match,
        op: &Op,
        max_matches: &Option<usize>,
    ) -> Result<(), ExecutionError> {
        match op {
            Op::Filter(regex) => {
                if regex.match_first(rope, &m.range).is_some() {
                    new_matches.push(m.clone());
                }
            }
            Op::FilterOut(regex) => {
                if !regex.match_first(rope, &m.range).is_some() {
                    new_matches.push(m.clone());
                }
            }
            Op::Extract(regex) => {
                new_matches.extend(regex.match_all(rope, &m.range, max_matches).drain(..).map(
                    |(range, captures)| Match {
                        range,
                        captures: Some(captures),
                    },
                ));
            }
            Op::ExtractReverse(regex) => {
                let mut front = m.range.front().clone();
                for (next, _) in regex.match_all(rope, &m.range, max_matches) {
                    let range = Range::from_points(front, *next.front());
                    if !range.is_empty() {
                        new_matches.push(Match {
                            range,
                            captures: None,
                        });
                    }
                    front = *next.back();
                }

                let range = Range::from_points(front, *m.range.back());
                if !range.is_empty() {
                    new_matches.push(Match {
                        range,
                        captures: None,
                    });
                }
            }
            Op::SurroundWithOutDelim(regex) => {
                // TODO:
            }
            Op::SurroundWithDelim(regex) => {
                // TODO:
            }
            Op::Prepend(text) => {
                rope.insert(m.range.front(), &text);
                new_matches.push(Match {
                    range: m.range.clone(),
                    captures: None,
                });
            }
            Op::Append(text) => {
                rope.insert(m.range.back(), &text);
                new_matches.push(Match {
                    range: m.range.clone(),
                    captures: None,
                });
            }
            Op::ReplaceWith(text) => {
                rope.remove(&m.range);
                rope.insert(m.range.front(), &text);
                new_matches.push(Match {
                    range: Range::from_points(*m.range.front(), *m.range.front()),
                    captures: None,
                });
            }
            Op::Delete => {
                rope.remove(&m.range);
                new_matches.push(Match {
                    range: Range::from_points(*m.range.front(), *m.range.front()),
                    captures: None,
                });
            }
            Op::Jump(to) => {
                // TODO:
            }
            Op::ShellCommand(cmd) => {
                let mut child = Command::new("bash")
                    .args(&["-c", &cmd])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .stdin(Stdio::piped())
                    .spawn()
                    .map_err(ExecutionError::CommandError)?;

                let mut stdin = child.stdin.as_mut().unwrap();
                for chunk in rope.sub_str(&m.range).chunks() {
                    stdin
                        .write_all(chunk.as_bytes())
                        .map_err(ExecutionError::CommandError)?;
                }

                let output = child
                    .wait_with_output()
                    .map_err(ExecutionError::CommandError)?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                if output.status.success() {
                    rope.remove(&m.range);
                    rope.insert(m.range.front(), &stdout);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).into();
                    return Err(ExecutionError::ExitedWith { stderr });
                }
            }
        }

        Ok(())
    }
}

fn build_regex(pattern: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(false)
        .ignore_whitespace(false)
        .multi_line(false)
        .build()
        .map(|regex| regex.into())
}

struct Parser<'a> {
    query: &'a str,
    iter: Peekable<Chars<'a>>,
    cursor: usize,
}

impl<'a> Parser<'a> {
    pub fn new(query: &str) -> Parser<'_> {
        Parser {
            query,
            iter: query.chars().peekable(),
            cursor: 0,
        }
    }

    fn skip_whitespaces(&mut self) {
        while matches!(self.peek(), Some(ch) if ch.is_ascii_whitespace()) {
            self.consume();
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Query>, ParseError> {
        let mut queries = Vec::new();
        loop {
            self.skip_whitespaces();
            let addr = match self.parse_addr()? {
                Some(addr) => addr,
                None => Address::Current,
            };

            self.skip_whitespaces();
            let op = match self.consume() {
                Some('g') => Op::Filter(self.parse_regex()?),
                Some('v') => Op::FilterOut(self.parse_regex()?),
                Some('x') => Op::Extract(self.parse_regex()?),
                Some('y') => Op::ExtractReverse(self.parse_regex()?),
                Some('s') => Op::SurroundWithOutDelim(self.parse_pattern()?),
                Some('S') => Op::SurroundWithDelim(self.parse_pattern()?),
                Some('i') => Op::Prepend(self.parse_string()?),
                Some('a') => Op::Append(self.parse_string()?),
                Some('c') => Op::ReplaceWith(self.parse_string()?),
                Some('d') => Op::Delete,
                Some('p') => Op::ShellCommand(self.parse_string()?),
                Some('j') => Op::Jump(match self.peek() {
                    Some('^') => {
                        self.consume();
                        JumpTo::Beginning
                    }
                    Some('|') => {
                        self.consume();
                        JumpTo::AfterIndent
                    }
                    Some('$') => {
                        self.consume();
                        JumpTo::End
                    }
                    _ => JumpTo::FirstMatch,
                }),
                Some(opcode) => {
                    return Err(ParseError {
                        cursor: self.last_consumed_cursor(),
                        kind: ParseErrorKind::UnknownOpcode { opcode },
                    });
                }
                None => {
                    if queries.is_empty() {
                        queries.push(Query {
                            addr,
                            op: Op::Jump(JumpTo::FirstMatch),
                        });
                    }
                    break;
                }
            };

            queries.push(Query { addr, op });
        }

        Ok(queries)
    }

    fn peek(&mut self) -> Option<char> {
        self.iter.peek().copied()
    }

    fn last_consumed_cursor(&self) -> usize {
        debug_assert!(self.cursor > 0);
        self.cursor - 1
    }

    fn consume(&mut self) -> Option<char> {
        let ch = self.iter.next();
        if ch.is_some() {
            self.cursor += 1;
        }
        ch
    }

    fn reset_iter(&mut self, cursor: usize) {
        self.cursor = 0;
        self.iter = self.query.chars().peekable();
        for _ in 0..cursor {
            self.consume();
        }
    }

    fn parse_simple_addr(&mut self) -> Result<Option<Address>, ParseError> {
        Ok(match self.peek() {
            Some('.') => {
                self.consume();
                Some(Address::Current)
            }
            Some('$') => {
                self.consume();
                Some(Address::EOF)
            }
            Some('/') => Some(Address::Match(self.parse_regex()?)),
            Some(ch) if ch.is_ascii_digit() => {
                let mut n = 0;
                while let Some(ch) = self.peek() {
                    if !ch.is_ascii_digit() {
                        break;
                    }

                    n = 10 * n + ch.to_digit(10).unwrap();
                    self.consume();
                }

                Some(Address::LineNo(n as usize))
            }
            _ => None,
        })
    }

    fn parse_offset_addr(&mut self) -> Result<Option<Address>, ParseError> {
        let prev_cursor = self.cursor;
        let mut from = self.parse_simple_addr()?;
        loop {
            let addr = match self.peek() {
                Some('+') => {
                    self.consume();
                    let end = self.parse_simple_addr()?;
                    Some(Address::Forward {
                        from: Box::new(from.unwrap_or(Address::Current)),
                        addr: Box::new(end.unwrap_or(Address::LineNo((1)))),
                    })
                }
                Some('-') => {
                    self.consume();
                    let end = self.parse_simple_addr()?;
                    Some(Address::Backward {
                        from: Box::new(from.unwrap_or(Address::Current)),
                        addr: Box::new(end.unwrap_or(Address::LineNo((1)))),
                    })
                }
                _ => {
                    return Ok(from);
                }
            };

            from = addr;
        }
    }

    fn parse_range_addr(&mut self) -> Result<Option<Address>, ParseError> {
        let prev_cursor = self.cursor;
        let start = self.parse_offset_addr()?;
        Ok(match self.peek() {
            Some(',') => {
                self.consume();
                let end = self.parse_offset_addr()?;
                Some(Address::Range {
                    start: Box::new(start.unwrap_or(Address::LineNo(0))),
                    end: Box::new(end.unwrap_or(Address::EOF)),
                })
            }
            _ => {
                self.reset_iter(prev_cursor);
                self.parse_offset_addr()?
            }
        })
    }

    fn parse_addr(&mut self) -> Result<Option<Address>, ParseError> {
        return self.parse_range_addr();
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        let delim = match self.consume() {
            Some(delim) => delim,
            None => {
                return Err(ParseError {
                    cursor: self.last_consumed_cursor(),
                    kind: ParseErrorKind::EmptyRegex,
                })
            }
        };

        let mut s = String::new();
        while let Some(ch) = self.consume() {
            if ch == delim {
                break;
            }

            if ch == '\\' && self.peek() == Some(delim) {
                self.consume();
                s.push(delim);
            } else {
                s.push(ch);
            }
        }

        Ok(s)
    }

    fn parse_pattern(&mut self) -> Result<Regex, ParseError> {
        match self.peek() {
            Some('/') => self.parse_regex(),
            Some(ch) => build_regex(&ch.to_string()).map_err(|err| ParseError {
                cursor: self.last_consumed_cursor(),
                kind: ParseErrorKind::InvalidRegex(err),
            }),
            None => Err(ParseError {
                cursor: self.cursor,
                kind: ParseErrorKind::EmptyRegex,
            }),
        }
    }

    fn parse_regex(&mut self) -> Result<Regex, ParseError> {
        let pattern = self.parse_string()?;
        if pattern.is_empty() {
            return Err(ParseError {
                cursor: self.last_consumed_cursor(),
                kind: ParseErrorKind::EmptyRegex,
            });
        }

        build_regex(&pattern).map_err(|err| ParseError {
            cursor: self.last_consumed_cursor(),
            kind: ParseErrorKind::InvalidRegex(err),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    fn parse(query: &str) -> Result<Vec<Query>, ParseError> {
        Parser::new(query).parse()
    }

    #[test]
    fn test_char_index_to_range() {
        assert_eq!(
            char_index_to_range(&Rope::from_str("abc"), &Range::new(0, 0, 0, 3), 0, 3),
            Range::new(0, 0, 0, 3)
        );

        assert_eq!(
            char_index_to_range(
                &Rope::from_str("abc\ndef\nxyz"),
                //                     ^^^^^
                &Range::new(1, 0, 2, 3),
                1,
                5
            ),
            Range::new(1, 1, 2, 1)
        );
    }

    #[test]
    fn test_parser() {
        assert_eq!(
            parse(""),
            Ok(vec![Query {
                addr: Address::Current,
                op: Op::Jump(JumpTo::FirstMatch)
            }])
        );

        assert_eq!(
            parse("x/a?c/"),
            Ok(vec![Query {
                addr: Address::Current,
                op: Op::Extract(Regex::new("a?c").unwrap())
            }])
        );

        assert_eq!(
            parse(",x/a?c/"),
            Ok(vec![Query {
                addr: Address::Range {
                    start: Box::new(Address::LineNo(0)),
                    end: Box::new(Address::EOF),
                },
                op: Op::Extract(Regex::new("a?c").unwrap())
            }])
        );

        assert_eq!(
            parse("x/\\//"),
            Ok(vec![Query {
                addr: Address::Current,
                op: Op::Extract(Regex::new("/").unwrap())
            }])
        );

        assert_eq!(
            parse("x/a?c/c/xyz"),
            Ok(vec![
                Query {
                    addr: Address::Current,
                    op: Op::Extract(Regex::new("a?c").unwrap()),
                },
                Query {
                    addr: Address::Current,
                    op: Op::ReplaceWith("xyz".to_owned()),
                }
            ])
        );

        assert_eq!(
            parse("+1-2"),
            Ok(vec![Query {
                addr: Address::Backward {
                    from: Box::new(Address::Forward {
                        from: Box::new(Address::Current),
                        addr: Box::new(Address::LineNo(1)),
                    }),
                    addr: Box::new(Address::LineNo(2)),
                },
                op: Op::Jump(JumpTo::FirstMatch),
            },])
        );

        assert_eq!(
            parse("+1-2+3"),
            Ok(vec![Query {
                addr: Address::Forward {
                    from: Box::new(Address::Backward {
                        from: Box::new(Address::Forward {
                            from: Box::new(Address::Current),
                            addr: Box::new(Address::LineNo(1)),
                        }),
                        addr: Box::new(Address::LineNo(2)),
                    }),
                    addr: Box::new(Address::LineNo(3)),
                },
                op: Op::Jump(JumpTo::FirstMatch),
            },])
        );

        assert_eq!(
            parse("+-"),
            Ok(vec![Query {
                addr: Address::Backward {
                    from: Box::new(Address::Forward {
                        from: Box::new(Address::Current),
                        addr: Box::new(Address::LineNo(1)),
                    }),
                    addr: Box::new(Address::LineNo(1)),
                },
                op: Op::Jump(JumpTo::FirstMatch),
            },])
        );

        assert_eq!(
            parse("$-/a?c/"),
            Ok(vec![Query {
                addr: Address::Backward {
                    from: Box::new(Address::EOF),
                    addr: Box::new(Address::Match(Regex::new("a?c").unwrap())),
                },
                op: Op::Jump(JumpTo::FirstMatch),
            },])
        );

        assert_eq!(
            parse(",x/int/ +-i/unsigned/"),
            Ok(vec![
                Query {
                    addr: Address::Range {
                        start: Box::new(Address::LineNo(0)),
                        end: Box::new(Address::EOF),
                    },
                    op: Op::Extract(Regex::new("int").unwrap()),
                },
                Query {
                    addr: Address::Backward {
                        from: Box::new(Address::Forward {
                            from: Box::new(Address::Current),
                            addr: Box::new(Address::LineNo(1)),
                        }),
                        addr: Box::new(Address::LineNo(1)),
                    },
                    op: Op::Prepend("unsigned".to_owned()),
                }
            ])
        );

        assert_eq!(
            parse(".+-j$a/;"),
            Ok(vec![
                Query {
                    addr: Address::Backward {
                        from: Box::new(Address::Forward {
                            from: Box::new(Address::Current),
                            addr: Box::new(Address::LineNo(1)),
                        }),
                        addr: Box::new(Address::LineNo(1)),
                    },
                    op: Op::Jump(JumpTo::End),
                },
                Query {
                    addr: Address::Current,
                    op: Op::Append(";".to_owned()),
                }
            ])
        );
    }

    #[test]
    fn invalid_inputs() {
        assert_eq!(
            parse("x/"),
            Err(ParseError {
                kind: ParseErrorKind::EmptyRegex,
                cursor: 1,
            })
        );
    }

    use crate::buffer::Buffer;
    fn run(text: &str, query: &str) -> Result<Buffer, ParseError> {
        let mut buffer = Buffer::from_str(text);
        buffer.set_cursor(Cursor::new(0, 0));
        let current = buffer.cursor_as_range();
        Engine::new(query)?.execute(buffer.rope_mut(), &current, None);
        Ok(buffer)
    }

    #[test]
    fn match_and_replace() {
        let buffer = run("abc", "/b/ c/X/").unwrap();
        assert_eq!(buffer.text(), "aXc");
        //        assert_eq!(buffer.cursor(), &Cursor::new(0, 2));

        let buffer = run("abcd", "/b.*/ c/X/").unwrap();
        assert_eq!(buffer.text(), "aX");
        //        assert_eq!(buffer.cursor(), &Cursor::new(0, 2));
    }

    #[test]
    fn match_reverse_and_replace() {
        let buffer = run("a123b", ",y/[1-9]+/ c/./").unwrap();
        assert_eq!(buffer.text(), ".123.");
        //        assert_eq!(buffer.cursor(), &Cursor::new(0, 1));

        let buffer = run("a1b2c3d", ",y/[1-9]/ c/./").unwrap();
        assert_eq!(buffer.text(), ".1.2.3.");
        //        assert_eq!(buffer.cursor(), &Cursor::new(0, 1));

        let buffer = run("", ",y/.*/ c//").unwrap();
        assert_eq!(buffer.text(), "");
        //        assert_eq!(buffer.cursor(), &Cursor::new(0, 0));
    }

    #[test]
    fn match_and_remove_everything() {
        let buffer = run("abc", ",x/./c/").unwrap();
        assert_eq!(buffer.text(), "");

        let buffer = run("a\nb", ",x/[^A]/c/").unwrap();
        assert_eq!(buffer.text(), "");
    }

    #[test]
    fn shell_command() {
        let buffer = run("abcde", "/b../ p#tr '[a-z]' '[A-Z]'#").unwrap();
        assert_eq!(buffer.text(), "aBCDe");
        //        assert_eq!(buffer.cursor(), &Cursor::new(0, 4));
    }

    #[bench]
    fn ned_on_small_text(b: &mut Bencher) {
        let mut buffer = Buffer::new();
        for _ in 0..100 {
            buffer.insert("0123456789");
        }

        let mut engine = Engine::new(",x/7/").unwrap();
        buffer.set_cursor(Cursor::new(0, 0));
        let current = buffer.cursor_as_range();

        b.iter(|| {
            engine.execute(buffer.rope_mut(), &current, None);
        });
    }

    #[bench]
    fn ned_on_large_text(b: &mut Bencher) {
        let mut buffer = Buffer::new();
        for _ in 0..1000 {
            buffer.insert("0123456789");
        }

        let mut engine = Engine::new(",x/./").unwrap();
        buffer.set_cursor(Cursor::new(0, 0));
        let current = buffer.cursor_as_range();

        b.iter(|| {
            engine.execute(buffer.rope_mut(), &current, None);
        });
    }

    #[bench]
    fn ned_on_large_text_with_match_limit(b: &mut Bencher) {
        let mut buffer = Buffer::new();
        for _ in 0..100000 {
            buffer.insert("0123456789");
        }

        let mut engine = Engine::new(",x/./").unwrap();
        buffer.set_cursor(Cursor::new(0, 0));
        let current = buffer.cursor_as_range();

        b.iter(|| {
            engine.execute(buffer.rope_mut(), &current, Some(100));
        });
    }
}
