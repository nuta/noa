use crate::rope::{Cursor, Range, Rope};
use regex::{Captures, Regex, RegexBuilder};
use std::iter::Peekable;
use std::str::Chars;

struct Match<'a> {
    captures: Captures<'a>,
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

#[derive(Debug)]
enum Op {
    /// `x`
    Extract { regex: Regex },
    /// `j`
    Jump(JumpTo),
}

#[derive(Debug)]
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
                    ops.push(Op::Extract {
                        regex: self.parse_regex()?,
                    });
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
            .map_err(|err| ParseError {
                cursor: self.last_consumed_cursor(),
                kind: ParseErrorKind::InvalidRegex(err),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Query {
        // Implement our own partial_eq since Regex is not PartialEq intentionally.
        fn assert_eq(&self, other: &Query) {
            assert_eq!(self.addr, other.addr);
            dbg!(&self.ops, &other.ops);
            assert_eq!(self.ops.len(), other.ops.len());
            for (i, (op1, op2)) in self.ops.iter().zip(other.ops.iter()).enumerate() {
                match (op1, op2) {
                    (Op::Extract { regex: regex1 }, Op::Extract { regex: regex2 }) => {
                        assert_eq!(regex1.as_str(), regex2.as_str());
                    }
                    (Op::Jump(jump1), Op::Jump(jump2)) => {
                        assert_eq!(jump1, jump2);
                    }
                    (_, _) => {
                        panic!("{}-th elements are not equal", i);
                    }
                }
            }
        }
    }

    fn parse(query: &str) -> Query {
        Parser::new(query).parse().unwrap()
    }

    fn failing_parse(query: &str) -> ParseError {
        Parser::new(query).parse().unwrap_err()
    }

    #[test]
    fn test_parse_addr() {
        parse("").assert_eq(&Query {
            addr: Address::Forward {
                start: Box::new(Address::Current),
                end: Box::new(Address::EOF),
            },
            ops: vec![Op::Jump(JumpTo::FirstMatch)],
        });

        parse("x/a?c/").assert_eq(&Query {
            addr: Address::Forward {
                start: Box::new(Address::Current),
                end: Box::new(Address::EOF),
            },
            ops: vec![Op::Extract {
                regex: Regex::new("a?c").unwrap(),
            }],
        });

        // assert_eq!(parse_addr("."), Ok((1, Some(Address::Current))));
        // assert_eq!(parse_addr("$"), Ok((1, Some(Address::EOF))));
        // assert_eq!(parse_addr("0"), Ok((1, Some(Address::LineNo(0)))));
        // assert_eq!(parse_addr("1"), Ok((1, Some(Address::LineNo(1)))));
        // assert_eq!(parse_addr("123"), Ok((3, Some(Address::LineNo(123)))));
        // assert_eq!(
        //     parse_addr("12,34"),
        //     Ok((
        //         5,
        //         Some(Address::Range {
        //             start: Box::new(Address::LineNo((12))),
        //             end: Box::new(Address::LineNo((34))),
        //         })
        //     ))
        // );
        // assert_eq!(
        //     parse_addr("12,"),
        //     Ok((
        //         3,
        //         Some(Address::Range {
        //             start: Box::new(Address::LineNo((12))),
        //             end: Box::new(Address::EOF),
        //         })
        //     ))
        // );
        // assert_eq!(
        //     parse_addr(",34"),
        //     Ok((
        //         3,
        //         Some(Address::Range {
        //             start: Box::new(Address::LineNo(0)),
        //             end: Box::new(Address::LineNo((34))),
        //         })
        //     ))
        // );
        // assert_eq!(
        //     parse_addr(","),
        //     Ok((
        //         1,
        //         Some(Address::Range {
        //             start: Box::new(Address::LineNo(0)),
        //             end: Box::new(Address::EOF),
        //         })
        //     ))
        // );
    }

    #[test]
    fn opcode_x() {}
}
