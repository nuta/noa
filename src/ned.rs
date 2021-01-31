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
        start: Box<Address>,
        end: Box<Address>,
    },
    /// `a1+a2`
    Backward {
        start: Box<Address>,
        end: Box<Address>,
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
    /// `f`
    SearchForward(Regex),
    /// `b`
    SearchBackward(Regex),
    /// `s`
    SurroundWithOutDelim(Regex),
    /// `S`
    SurroundWithDelim(Regex),
    /// `i`
    Prepend(String),
    /// `a`
    Append(String),
    /// `r`
    ReplaceWith(String),
    /// `d`
    Delete,
    /// `j`
    Jump(JumpTo),
    /// `p`
    ShellCommand(String),
    /// `c`
    Lowercase,
    /// `C`
    Upcase,
}

#[derive(Debug, PartialEq)]
struct Query {
    addr: Address,
    ops: Vec<Op>,
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
    query: Query,
    cursor: Range,
}

impl Engine {
    pub fn new(query: &str) -> Result<Engine, ParseError> {
        let mut parser = Parser::new(query);
        Ok(Engine {
            query: parser.parse()?,
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

    pub fn parse(&mut self) -> Result<Query, ParseError> {
        let addr = match self.parse_addr()? {
            Some(addr) => addr,
            None => Address::Forward {
                start: Box::new(Address::Current),
                end: Box::new(Address::EOF),
            },
        };

        let mut ops = Vec::new();
        loop {
            while matches!(self.peek(), Some(ch) if ch.is_ascii_whitespace()) {
                self.consume();
            }

            match self.consume() {
                Some('g') => {
                    ops.push(Op::Filter(self.parse_regex()?));
                }
                Some('v') => {
                    ops.push(Op::FilterOut(self.parse_regex()?));
                }
                Some('x') => {
                    ops.push(Op::Extract(self.parse_regex()?));
                }
                Some('y') => {
                    ops.push(Op::ExtractReverse(self.parse_regex()?));
                }
                Some('f') => {
                    ops.push(Op::SearchForward(self.parse_pattern()?));
                }
                Some('b') => {
                    ops.push(Op::SearchBackward(self.parse_pattern()?));
                }
                Some('s') => {
                    ops.push(Op::SurroundWithOutDelim(self.parse_pattern()?));
                }
                Some('S') => {
                    ops.push(Op::SurroundWithDelim(self.parse_pattern()?));
                }
                Some('i') => {
                    ops.push(Op::Prepend(self.parse_string()?));
                }
                Some('a') => {
                    ops.push(Op::Append(self.parse_string()?));
                }
                Some('r') => {
                    ops.push(Op::ReplaceWith(self.parse_string()?));
                }
                Some('d') => {
                    ops.push(Op::Delete);
                }
                Some('p') => {
                    ops.push(Op::ShellCommand(self.parse_string()?));
                }
                Some('c') => {
                    ops.push(Op::Lowercase);
                }
                Some('C') => {
                    ops.push(Op::Upcase);
                }
                Some('j') => {
                    ops.push(Op::Jump(match self.peek() {
                        Some('^') => { self.consume(); JumpTo::Beginning }
                        Some('|') => { self.consume(); JumpTo::AfterIndent }
                        Some('$') => { self.consume(); JumpTo::End }
                        _ => JumpTo::FirstMatch,
                    }));
                }
                Some(opcode) => {
                    return Err(ParseError {
                        cursor: self.last_consumed_cursor(),
                        kind: ParseErrorKind::UnknownOpcode { opcode },
                    });
                }
                None => {
                    break;
                }
            }
        }

        if ops.is_empty() {
            ops.push(Op::Jump(JumpTo::FirstMatch));
        }

        Ok(Query { addr, ops })
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

    fn parse_addr(&mut self) -> Result<Option<Address>, ParseError> {
        let addr1 = match self.peek() {
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
        };

        // Handle compound addresses.
        let addr = match self.peek() {
            Some(',') => {
                self.consume();
                let end = self.parse_addr()?;
                Some(Address::Range {
                    start: Box::new(addr1.unwrap_or(Address::LineNo(0))),
                    end: Box::new(end.unwrap_or(Address::EOF)),
                })
            }
            Some('+') => {
                self.consume();
                let end = self.parse_addr()?;
                Some(Address::Forward {
                    start: Box::new(addr1.unwrap_or(Address::Current)),
                    end: Box::new(end.unwrap_or(Address::EOF)),
                })
            }
            Some('-') => {
                self.consume();
                let end = self.parse_addr()?;
                Some(Address::Backward {
                    start: Box::new(addr1.unwrap_or(Address::Current)),
                    end: Box::new(end.unwrap_or(Address::LineNo(0))),
                })
            }
            _ => addr1,
        };

        Ok(addr)
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

    fn parse(query: &str) -> Result<Query, ParseError> {
        Parser::new(query).parse()
    }

    #[test]
    fn test_parser() {
        assert_eq!(
            parse(""),
            Ok(Query {
                addr: Address::Forward {
                    start: Box::new(Address::Current),
                    end: Box::new(Address::EOF),
                },
                ops: vec![Op::Jump(JumpTo::FirstMatch)],
            })
        );

        assert_eq!(
            parse("x/a?c/"),
            Ok(Query {
                addr: Address::Forward {
                    start: Box::new(Address::Current),
                    end: Box::new(Address::EOF),
                },
                ops: vec![Op::Extract(Regex::new("a?c").unwrap())]
            })
        );

        assert_eq!(
            parse(",x/a?c/"),
            Ok(Query {
                addr: Address::Range {
                    start: Box::new(Address::LineNo(0)),
                    end: Box::new(Address::EOF),
                },
                ops: vec![Op::Extract(Regex::new("a?c").unwrap())]
            })
        );

        assert_eq!(
            parse("x/\\//"),
            Ok(Query {
                addr: Address::Forward {
                    start: Box::new(Address::Current),
                    end: Box::new(Address::EOF),
                },
                ops: vec![Op::Extract(Regex::new("/").unwrap())]
            })
        );

        assert_eq!(
            parse("x/a?c/r/xyz"),
            Ok(Query {
                addr: Address::Forward {
                    start: Box::new(Address::Current),
                    end: Box::new(Address::EOF),
                },
                ops: vec![
                    Op::Extract(Regex::new("a?c").unwrap()),
                    Op::ReplaceWith("xyz".to_owned()),
                ],
            })
        );

        assert_eq!(
            parse(".+-j$a/;"),
            Ok(Query {
                addr: Address::Forward {
                    start: Box::new(Address::Current),
                    end: Box::new(Address::EOF),
                },
                ops: vec![
                    Op::Jump(JumpTo::End),
                    Op::Append(";".to_owned()),
                ],
            })
        );
    }
}
