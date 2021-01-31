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
    // `.`
    Current,
    // `0`, `123`, ...
    LineNo(usize),
    // `/.../`
    Match(Regex),
    // `$`
    EOF,
    // `a1,a2`
    Range {
        start: Box<Address>,
        end: Box<Address>,
    },
    // `a1+a2`
    Forward {
        start: Box<Address>,
        end: Box<Address>,
    },
}

#[derive(Debug, PartialEq)]
enum JumpTo {
    FirstMatch,
}

#[derive(Debug, PartialEq)]
enum Op {
    /// `x`
    Extract(Regex),
    /// `j`
    Jump(JumpTo),
    /// `r`
    ReplaceWith(String),
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
            match self.consume() {
                Some('x') => {
                    ops.push(Op::Extract(self.parse_regex()?));
                }
                Some('r') => {
                    ops.push(Op::ReplaceWith(self.parse_string()?));
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

            s.push(ch);
        }

        Ok(s)
    }

    fn parse_regex(&mut self) -> Result<Regex, ParseError> {
        let delim = match self.consume() {
            Some(delim) => delim,
            None => {
                return Err(ParseError {
                    cursor: self.last_consumed_cursor(),
                    kind: ParseErrorKind::EmptyRegex,
                })
            }
        };

        let mut regex_str = String::new();
        while let Some(ch) = self.consume() {
            if ch == delim {
                break;
            }

            if ch == '\\' && self.peek() == Some(delim) {
                self.consume();
                regex_str.push(delim);
            } else {
                regex_str.push(ch);
            }
        }

        RegexBuilder::new(&regex_str)
            .case_insensitive(false)
            .ignore_whitespace(false)
            .multi_line(false)
            .build()
            .map(|regex| regex.into())
            .map_err(|err| ParseError {
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
    }
}
