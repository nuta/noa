use std::fs::OpenOptions;
use std::path::Path;

use ropey::iter::Chunks;

use crate::cursor::{Point, Range};

#[derive(Clone, Debug, PartialEq)]
pub struct Rope {
    inner: ropey::Rope,
    modified_line: Option<usize>,
    cached_num_lines: usize,
    version: usize,
}

impl Rope {
    pub fn new() -> Rope {
        Rope {
            inner: ropey::Rope::new(),
            modified_line: None,
            cached_num_lines: 1,
            version: 1,
        }
    }

    pub fn from_reader<T: std::io::Read>(reader: T) -> std::io::Result<Rope> {
        let inner = ropey::Rope::from_reader(reader)?;
        let cached_num_lines = inner.len_lines();
        Ok(Rope {
            inner,
            modified_line: None,
            cached_num_lines,
            version: 1,
        })
    }

    #[cfg(test)]
    pub fn from_str(str: &str) -> Rope {
        let mut rope = Rope::new();
        rope.insert(Point::new(0, 0), str);
        rope
    }

    pub fn is_empty(&self) -> bool {
        self.inner.len_chars() == 0
    }

    /// Returns the number of characters in the buffer.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.len_chars()
    }

    /// Returns the number of bytes in the buffer.
    pub fn len_bytes(&self) -> usize {
        self.inner.len_bytes()
    }

    /// Returns the number of characters in the buffer.
    pub fn num_lines(&self) -> usize {
        self.cached_num_lines
    }

    /// Returns the number of characters in a line except new line characters.
    pub fn line_len(&self, line: usize) -> usize {
        if line == self.num_lines() {
            0
        } else {
            self.line(line).len_chars()
        }
    }

    pub fn chunks(&self) -> Chunks {
        self.inner.chunks()
    }

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn modified_line(&self) -> &Option<usize> {
        &self.modified_line
    }

    pub fn reset_modified_line(&mut self) {
        self.modified_line = None;
    }

    pub fn text(&self) -> String {
        self.inner.to_string()
    }

    pub fn save_into_file(&self, path: &Path) -> std::io::Result<()> {
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        self.inner.write_to(f)
    }

    pub fn insert(&mut self, pos: Point, string: &str) {
        self.inner.insert(self.index_in_rope(pos), string);
        self.on_modified(pos.y);
    }

    pub fn insert_char(&mut self, pos: Point, ch: char) {
        self.inner.insert_char(self.index_in_rope(pos), ch);
        self.on_modified(pos.y);
    }

    pub fn clear(&mut self) {
        self.inner.remove(0..self.inner.len_chars());
    }

    pub fn remove(&mut self, range: &Range) {
        let start = self.index_in_rope(range.front());
        let end = self.index_in_rope(range.back());
        self.inner.remove(start..end);
        self.on_modified(range.front().y);
    }

    fn on_modified(&mut self, start_y: usize) {
        self.cached_num_lines = self.inner.len_lines();
        self.modified_line = Some(start_y);
        self.version += 1;
    }

    /// Returns a line except new line characters.
    pub fn line(&self, line: usize) -> ropey::RopeSlice {
        let slice = self.inner.line(line);

        // The slice contains newline characters. Trim them.
        let mut len = slice.len_chars();
        while len > 0 {
            if slice.char(len - 1) != '\n' {
                break;
            }

            len -= 1;
        }

        slice.slice(..len)
    }

    pub fn chars(&self) -> ropey::iter::Chars<'_> {
        self.inner.chars()
    }

    pub fn sub_str(&self, range: &Range) -> ropey::RopeSlice {
        let start = self.index_in_rope(range.front());
        let end = self.index_in_rope(range.back());
        self.inner.slice(start..end)
    }

    pub fn word_at(&self, pos: &Point) -> Option<(Range, String)> {
        if pos.y >= self.num_lines() {
            return None;
        }

        let mut word = String::new();
        let mut start = 0;
        let mut end = 0;
        for (i, ch) in self.line(pos.y).chars().enumerate() {
            if is_word_char(ch) {
                word.push(ch);
                end = i + 1;
            } else if i >= pos.x {
                break;
            } else {
                word.clear();
                start = i + 1;
            }
        }

        if word.is_empty() {
            None
        } else {
            Some((Range::new(pos.y, start, pos.y, end), word))
        }
    }

    pub fn prev_word_at(&self, pos: &Point) -> Option<Range> {
        if pos.x == 0 {
            if pos.y > 0 {
                return Some(Range::new(pos.y - 1, self.line_len(pos.y - 1), pos.y, 0));
            } else {
                return None;
            }
        }

        let mut state = None;
        let mut start = 0;
        for (i, ch) in self.line(pos.y).chars().take(pos.x).enumerate() {
            let current_state = is_word_char(ch);
            match state {
                Some(prev_state) if prev_state == current_state => {}
                _ => {
                    start = i;
                }
            }

            state = Some(current_state);
        }

        Some(Range::new(pos.y, start, pos.y, pos.x))
    }

    pub fn prev_word_end(&self, pos: &Point) -> Point {
        assert!(pos.y < self.num_lines());

        let line = self.line(pos.y).to_string();
        let line_len = line.chars().count();
        let end = line
            .chars()
            .rev()
            .skip(line_len - pos.x)
            .enumerate()
            .skip_while(|(_, ch)| !is_word_char(*ch))
            .skip_while(|(_, ch)| is_word_char(*ch))
            .next()
            .map(|(i, _)| pos.x - i)
            .unwrap_or(0);

        Point::new(pos.y, end)
    }

    pub fn next_word_end(&self, pos: &Point) -> Point {
        assert!(pos.y < self.num_lines());

        let line = self.line(pos.y);
        let end = line
            .chars()
            .skip(pos.x)
            .enumerate()
            .skip_while(|(_, ch)| !is_word_char(*ch))
            .skip_while(|(_, ch)| is_word_char(*ch))
            .next()
            .map(|(i, _)| pos.x + i)
            .unwrap_or_else(|| line.len_chars());

        Point::new(pos.y, end)
    }

    pub fn find(&self, needle: &str, after: Option<Point>) -> Option<Point> {
        if needle.is_empty() {
            return None;
        }

        let start = match after {
            Some(Point { y, x }) => {
                if y == self.num_lines() - 1 && x == self.line_len(y) {
                    // EOF.
                    return None;
                }

                self.point_to_byte_off(Point::new(y, x + 1))
            }
            None => 0,
        };

        // FIXME: Avoid using .text().
        let text = self.text();
        text[start..]
            .find(needle)
            .map(|off| self.byte_off_to_point(start + off))
    }

    fn index_in_rope(&self, pos: Point) -> usize {
        let x = if pos.x == std::usize::MAX {
            self.line_len(pos.y)
        } else {
            pos.x
        };

        self.inner.line_to_char(pos.y) + x
    }

    fn point_to_byte_off(&self, pos: Point) -> usize {
        let line_off = self.inner.line_to_byte(pos.y);
        let line = self.inner.line(pos.y);
        let column_off = line.char_to_byte(pos.x);
        line_off + column_off
    }

    fn byte_off_to_point(&self, offset: usize) -> Point {
        let lineno = self.inner.byte_to_line(offset);
        let line_off = self.inner.line_to_byte(lineno);
        let line = self.inner.line(lineno);
        let colno = line.byte_to_char(offset - line_off);
        Point::new(lineno, colno)
    }
}

fn is_word_char(ch: char) -> bool {
    char::is_ascii_alphanumeric(&ch) || ch == '_'
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn point_to_byte_off() {
        // ABC
        // あaア
        // xy
        //
        // (Both あ and ア occupy 3 bytes.)
        let rope = Rope::from_str("ABC\nあaア\nxy");
        assert_eq!(rope.point_to_byte_off(Point::new(0, 2)), 2);
        assert_eq!(rope.point_to_byte_off(Point::new(1, 0)), 4);
        assert_eq!(rope.point_to_byte_off(Point::new(1, 1)), 7);
        assert_eq!(rope.point_to_byte_off(Point::new(2, 2)), 14);
    }

    #[test]
    fn byte_off_to_point() {
        // ABC
        // あaア
        // xy
        //
        // (Both あ and ア occupy 3 bytes.)
        let rope = Rope::from_str("ABC\nあaア\nxy");
        assert_eq!(rope.byte_off_to_point(4), Point::new(1, 0));
        assert_eq!(rope.byte_off_to_point(2), Point::new(0, 2));
        assert_eq!(rope.byte_off_to_point(7), Point::new(1, 1));
        assert_eq!(rope.byte_off_to_point(14), Point::new(2, 2));
    }

    #[test]
    fn find() {
        // It's ABC
        // これはABCです
        // ABC
        //
        // (あ and its friends occupy 3 bytes.)
        let rope = Rope::from_str("It's ABC\nこれはABCです\nABC");
        assert_eq!(rope.find("", None), None);
        assert_eq!(rope.find("It's", None), Some(Point::new(0, 0)));
        assert_eq!(rope.find("ABC", None), Some(Point::new(0, 5)));
        assert_eq!(
            rope.find("ABC", Some(Point::new(0, 4))),
            Some(Point::new(0, 5))
        );
        assert_eq!(
            rope.find("ABC", Some(Point::new(0, 5))),
            Some(Point::new(1, 3))
        );
        assert_eq!(
            rope.find("ABC", Some(Point::new(1, 3))),
            Some(Point::new(2, 0))
        );
        assert_eq!(rope.find("ABC", Some(Point::new(2, 0))), None);
        assert_eq!(rope.find("ABC", Some(Point::new(2, 3))), None);
    }
}
