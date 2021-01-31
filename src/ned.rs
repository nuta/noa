use crate::rope::{Rope, Cursor, Range};
use regex::{Regex, Captures};

struct Match<'a> {
    captures: Captures<'a>,
}

struct InOut<'a> {
    matches: Vec<Match<'a>>,
}

enum Address {
    Current,
    All,
    Range {
        start: Box<Address>,
        end: Box<Address>,
    }
}

enum Op {
    /// `x`
    Extract,
}

struct Query {
    addr: Address,
    ops: Vec<Op>,
}

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

fn parse(cmd: &str) -> Result<Query, NedError> {
    addr = parse_addr();
    Ok(Query {})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser() {

    }

    #[test]
    fn opcode_x() {
    }
}
