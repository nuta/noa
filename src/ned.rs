use crate::rope::{Cursor, Range, Rope};
use regex::{Captures, Regex};
use std::iter::Peekable;
use std::str::Chars;

struct Match<'a> {
    captures: Captures<'a>,
}

struct InOut<'a> {
    matches: Vec<Match<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
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
        from: Box<Address>,
        until: Box<Address>,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum Op {
    /// `x`
    Extract,
}

#[derive(Debug, PartialEq, Eq)]
struct Query {
    addr: Address,
    ops: Vec<Op>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum NedError {
    ParseError,
}

pub struct Engine {
    query: Query,
    cursor: Range,
}

impl Engine {
    pub fn new(query: &str) -> Result<Engine, NedError> {
        Ok(Engine {
            query: parse(query)?,
            cursor: Range::new(0, 0, 0, 0),
        })
    }

    pub fn execute(&mut self, text: &str) -> Result<(), NedError> {
        Ok(())
    }

    pub fn execute_on_rope(&mut self, rope: &mut Rope) -> Result<(), NedError> {
        Ok(())
    }
}

fn parse_addr(cmd: &str) -> Result<(usize, Option<Address>), NedError> {
    let mut iter = cmd.char_indices().peekable();
    let addr1 = match iter.peek() {
        Some((_, '.')) => {
            iter.next();
            Some(Address::Current)
        }
        Some((_, '$')) => {
            iter.next();
            Some(Address::EOF)
        }
        Some((_, ch)) if ch.is_ascii_digit() => {
            let mut n = 0;
            while let Some((_, ch)) = iter.peek() {
                if !ch.is_ascii_digit() {
                    break;
                }

                n = 10 * n + ch.to_digit(10).unwrap();
                iter.next();
            }

            Some(Address::LineNo(n as usize))
        }
        _ => None,
    };

    // Handle compound addresses.
    let mut consumed_len = iter.clone().next().map(|(i, _)| i).unwrap_or(cmd.len());
    let rest = &cmd[consumed_len..];
    let addr = match iter.next() {
        Some((_, ',')) => {
            let (len, end) = parse_addr(&rest[1..])?;
            consumed_len += 1 + len;
            Some(Address::Range {
                start: Box::new(addr1.unwrap_or(Address::LineNo(0))),
                end: Box::new(end.unwrap_or(Address::EOF)),
            })
        }
        _ => addr1,
    };

    Ok((consumed_len, addr))
}

fn parse(cmd: &str) -> Result<Query, NedError> {
    let (consumed_len, addr) = match parse_addr(cmd)? {
        (consumed_len, Some(addr)) => (consumed_len, addr),
        (consumed_len, None) => (
            consumed_len,
            Address::Forward {
                from: Box::new(Address::Current),
                until: Box::new(Address::EOF),
            },
        ),
    };

    let mut ops = Vec::new();
    let rest = &cmd[consumed_len..];
    Ok(Query { addr, ops })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_addr() {
        assert_eq!(parse_addr(""), Ok((0, None)));
        assert_eq!(parse_addr("0"), Ok((1, Some(Address::LineNo(0)))));
        assert_eq!(parse_addr("1"), Ok((1, Some(Address::LineNo(1)))));
        assert_eq!(parse_addr("123"), Ok((3, Some(Address::LineNo(123)))));
        assert_eq!(
            parse_addr("12,34"),
            Ok((
                5,
                Some(Address::Range {
                    start: Box::new(Address::LineNo((12))),
                    end: Box::new(Address::LineNo((34))),
                })
            ))
        );
        assert_eq!(
            parse_addr("12,"),
            Ok((
                3,
                Some(Address::Range {
                    start: Box::new(Address::LineNo((12))),
                    end: Box::new(Address::EOF),
                })
            ))
        );
        assert_eq!(
            parse_addr(",34"),
            Ok((
                3,
                Some(Address::Range {
                    start: Box::new(Address::LineNo(0)),
                    end: Box::new(Address::LineNo((34))),
                })
            ))
        );
        assert_eq!(
            parse_addr(","),
            Ok((
                1,
                Some(Address::Range {
                    start: Box::new(Address::LineNo(0)),
                    end: Box::new(Address::EOF),
                })
            ))
        );
    }

    #[test]
    fn opcode_x() {}
}
