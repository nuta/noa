use crate::rope::{Cursor, Range, Rope};
use regex::{Captures, RegexBuilder};
use std::iter::Peekable;
use std::str::Chars;

struct Match<'a> {
    captures: Captures<'a>,
}

/// A wrapper of Regex to implement PartialEq to simplify unit tests.
#[derive(Debug)]
struct Regex(regex::Regex);

impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, regex::Error> {
        Ok(Regex(regex::Regex::new(pattern)?))
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

struct InOut<'a> {
    matches: Vec<Match<'a>>,
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

pub struct Engine {
    queries: Vec<Query>,
    cursor: Range,
}

impl Engine {
    pub fn new(query: &str) -> Result<Engine, ParseError> {
        let mut parser = Parser::new(query);
        Ok(Engine {
            queries: parser.parse()?,
            cursor: Range::new(0, 0, 0, 0),
        })
    }

    pub fn execute(&mut self, text: &str) -> Result<(), ParseError> {
        Ok(())
    }

    pub fn execute_on_rope(&mut self, rope: &mut Rope) -> Result<(), ParseError> {
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
                None => Address::Range {
                    start: Box::new(Address::Current),
                    end: Box::new(Address::EOF),
                },
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
                _ => { return Ok(from); }
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
        build_regex(&self.parse_string()?).map_err(|err| ParseError {
            cursor: self.last_consumed_cursor(),
            kind: ParseErrorKind::InvalidRegex(err),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(query: &str) -> Result<Vec<Query>, ParseError> {
        Parser::new(query).parse()
    }

    #[test]
    fn test_parser() {
        assert_eq!(
            parse(""),
            Ok(vec![Query {
                addr: Address::Range {
                    start: Box::new(Address::Current),
                    end: Box::new(Address::EOF),
                },
                op: Op::Jump(JumpTo::FirstMatch)
            }])
        );

        assert_eq!(
            parse("x/a?c/"),
            Ok(vec![Query {
                addr: Address::Range {
                    start: Box::new(Address::Current),
                    end: Box::new(Address::EOF),
                },
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
                addr: Address::Range {
                    start: Box::new(Address::Current),
                    end: Box::new(Address::EOF),
                },
                op: Op::Extract(Regex::new("/").unwrap())
            }])
        );

        assert_eq!(
            parse("x/a?c/c/xyz"),
            Ok(vec![
                Query {
                    addr: Address::Range {
                        start: Box::new(Address::Current),
                        end: Box::new(Address::EOF),
                    },
                    op: Op::Extract(Regex::new("a?c").unwrap()),
                },
                Query {
                    addr: Address::Range {
                        start: Box::new(Address::Current),
                        end: Box::new(Address::EOF),
                    },
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
                    addr: Address::Range {
                        start: Box::new(Address::Current),
                        end: Box::new(Address::EOF),
                    },
                    op: Op::Append(";".to_owned()),
                }
            ])
        );
    }
}
