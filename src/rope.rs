use std::fmt;
use std::fs::File;
use std::path::Path;
use std::cmp::{min, max};
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Point {
    pub y: usize,
    pub x: usize,
}

impl Point {
    pub fn new(y: usize, x: usize) -> Point {
        Point {
            y,
            x,
        }
    }

    pub fn move_by(
        &mut self,
        rope: &Rope,
        up: usize,
        down: usize,
        left: usize,
        right: usize
    ) {
        let num_lines = rope.num_lines();
        let mut r = right;
        loop {
            let max_x = rope.line_len(self.y);
            if self.x + r <= max_x {
                self.x += r;
                break;
            } else if self.y >= num_lines {
                break;
            } else {
                r -= max_x - self.x + 1;
                self.x = 0;
                self.y += 1;
            }
        }

        let mut l = left;
        loop {
            if l <= self.x {
                self.x -= l;
                break;
            } else if self.y == 0 {
                break;
            } else {
                l -= self.x;
                if l > 0 {
                    l -= 1;
                    self.y -= 1;
                    self.x = rope.line_len(self.y);
                }
            }
        }

        self.y = self.y.saturating_add(down);
        self.y = self.y.saturating_sub(up);

        self.y = min(self.y, num_lines);
        self.x = min(self.x, rope.line_len(self.y));
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.y, self.x)
    }
}

impl Ord for Point {
    fn cmp(&self, other: &Point) -> Ordering {
        let a = self;
        let b = other;
        if a == b {
            Ordering::Equal
        } else {
            match a.y.cmp(&b.y) {
                Ordering::Less => Ordering::Less,
                Ordering::Greater => Ordering::Greater,
                Ordering::Equal => {
                    if a.x < b.x {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                }
            }
        }
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Point) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Range {
    pub start: Point,
    pub end: Point,
}

impl Range {
    pub fn new(start_y: usize, start_x: usize, end_y: usize, end_x: usize) -> Range {
        Range::from_points(Point::new(start_y, start_x), Point::new(end_y, end_x))
    }

    pub fn from_points(start: Point, end: Point) -> Range {
        Range {
            start,
            end,
        }
    }

    pub fn front(&self) -> &Point {
        min(&self.start, &self.end)
    }

    pub fn back(&self) -> &Point {
        max(&self.start, &self.end)
    }

    pub fn overlaps_with(&self, other: &Range) -> bool {
        !(self.end.y < other.start.y
            || self.start.y > other.end.y
            || (self.end.y == other.start.y && self.end.x < other.start.x)
            || (self.start.y == other.end.y && self.start.x > other.end.x))
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Cursor {
    Normal {
        pos: Point,
    },
    Selection(Range),
}

impl Cursor {
    pub fn new(y: usize, x: usize) -> Cursor {
        Cursor::Normal {
            pos: Point::new(y, x),
        }
    }
}

impl Ord for Cursor {
    fn cmp(&self, other: &Cursor) -> Ordering {
        let a = match self {
            Cursor::Normal { pos, .. } => {
                pos
            }
            Cursor::Selection(Range { start, .. }) => {
                start
            }
        };

        let b = match other {
            Cursor::Normal { pos, .. } => {
                pos
            }
            Cursor::Selection(Range { start, .. }) => {
                start
            }
        };

        a.cmp(b)
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Cursor) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rope {
    inner: ropey::Rope,
    modified_line: Option<usize>,
    cached_num_lines: usize,
}

impl Rope {
    pub fn new() -> Rope {
        Rope {
            inner: ropey::Rope::new(),
            modified_line: None,
            cached_num_lines: 1,
        }
    }

    pub fn from_reader<T: std::io::Read>(
        mut reader: T
    ) -> std::io::Result<Rope> {
        let inner = ropey::Rope::from_reader(reader)?;
        let cached_num_lines = inner.len_lines();
        Ok(Rope {
            inner,
            modified_line: None,
            cached_num_lines,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.inner.len_chars() == 0
    }

    /// Returns the number of characters in the buffer.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.len_chars()
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
        self.inner.write_to(File::open(path)?)
    }

    pub fn insert(&mut self, pos: &Point, string: &str) {
        self.inner.insert(self.index_in_rope(pos), string);
        self.on_modified(pos.y);
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

    pub fn prev_word_end(&self, pos: &Point) -> Point {
        assert!(pos.y < self.num_lines());

        let line = self.line(pos.y).to_string();
        let line_len = line.chars().count();
        let end = line.chars()
            .rev()
            .skip(line_len - pos.x)
            .enumerate()
            .skip_while(|(_, ch)| !is_word_char(*ch))
            .skip_while(|(_, ch)| is_word_char(*ch))
            .next()
            .map(|(i, ch)| {
                pos.x - i
            })
            .unwrap_or(0);

        Point::new(pos.y, end)
    }

    pub fn next_word_end(&self, pos: &Point) -> Point {
        assert!(pos.y < self.num_lines());

        let line = self.line(pos.y);
        let end = line.chars()
            .skip(pos.x)
            .enumerate()
            .skip_while(|(_, ch)| !is_word_char(*ch))
            .skip_while(|(_, ch)| is_word_char(*ch))
            .next()
            .map(|(i, ch)| {
                pos.x + i
            })
            .unwrap_or_else(|| {
                line.len_chars()
            });

        Point::new(pos.y, end)
    }

    fn index_in_rope(&self, pos: &Point) -> usize {
        self.inner.line_to_char(pos.y) + pos.x
    }
}

fn is_word_char(ch: char) -> bool {
    char::is_ascii_alphanumeric(&ch) || ch == '_'
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::buffer::Buffer;

    #[test]
    fn sorting_cursors() {
        // a|bc|
        // de|f
        // x|yz
        let mut b = Buffer::new();
        b.insert("abc\ndef\nxyz");

        // Make sure cursors gets sorted.
        b.set_cursors(vec![
            Cursor::new(1, 2),
            Cursor::new(0, 3),
            Cursor::new(2, 1),
            Cursor::new(0, 1),
        ]);
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
            Cursor::new(0, 3),
            Cursor::new(1, 2),
            Cursor::new(2, 1),
        ]);
    }
}
