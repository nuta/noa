use ropey::Rope;

struct Position {
    line: usize,
    column: usize,
}

impl Position {
    fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

pub struct Cursor {
    pos: Position,
}

impl Cursor {
    fn new(pos: Position) -> Self {
        Self { pos }
    }
}

pub struct Buffer {
    rope: Rope,
    cursors: Vec<Cursor>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            cursors: vec![Cursor::new(Position::new(0, 0))],
        }
    }

    pub fn insert_str(&mut self, text: &str) {
        for c in self.cursors.iter_mut().rev() {
            // TODO:
        }
    }
}

impl ToString for Buffer {
    fn to_string(&self) -> String {
        self.rope.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_str() {
        let mut buffer = Buffer::new();
        buffer.insert_str("Hello, world!");
        assert_eq!(buffer.to_string(), "Hello, world!");
    }
}
