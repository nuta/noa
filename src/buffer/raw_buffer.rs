use crate::cursor::{Position, Range};

pub struct RawBuffer {
    pub rope: ropey::Rope,
    cached_num_lines: usize,
}

impl RawBuffer {
    pub fn new() -> RawBuffer {
        RawBuffer {
            rope: ropey::Rope::new(),
            cached_num_lines: 0,
        }
    }

    pub fn edit(&mut self, range: Range, new_text: &str) {
        let start = self.index_in_rope(range.front());
        let end = self.index_in_rope(range.back());
        self.rope.remove(start..end);
        self.rope.insert(start, new_text);
        self.cached_num_lines = self.rope.len_lines();
    }

    fn num_lines(&self) -> usize {
        self.cached_num_lines
    }

    /// Returns the number of characters in a line except new line characters.
    fn line_len(&self, line: usize) -> usize {
        if line == self.num_lines() {
            0
        } else {
            self.rope.line(line).len_chars()
        }
    }

    fn index_in_rope(&self, pos: Position) -> usize {
        let column = if pos.column == std::usize::MAX {
            self.line_len(pos.line)
        } else {
            pos.column
        };

        self.rope.line_to_char(pos.line) + column
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insertion() {
        let mut buffer = RawBuffer::new();
    }
}
