use std::cmp::min;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

impl Point {
    pub const fn new(y: usize, x: usize) -> Point {
        Point { x, y }
    }

    pub fn move_y_by(&mut self, diff: isize) {
        if diff < 0 {
            self.y = self.y.saturating_sub(diff.abs() as usize);
        } else {
            self.y += diff as usize;
        }
    }
}

impl fmt::Debug for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(y={}, x={})", self.y, self.x)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: Point,
    pub end: Point,
}

impl Range {
    pub const fn new(start: Point, end: Point) -> Range {
        Range { start, end }
    }

    pub const fn from_point(point: Point) -> Range {
        Range::new(point, point)
    }
}

impl fmt::Debug for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}, {:?}]", self.start, self.end)
    }
}

#[derive(Clone)]
pub struct Cursor {
    selection: Range,
}

impl Cursor {
    pub const fn new(position: Point) -> Cursor {
        Cursor {
            selection: Range::from_point(position)
        }
    }

    pub fn start(&self) -> &Point {
        &self.selection.start
    }

    pub fn start_mut(&mut self) -> &mut Point {
        &mut self.selection.start
    }

    pub fn end(&self) -> &Point {
        &self.selection.end
    }

    pub fn end_mut(&mut self) -> &mut Point {
        &mut self.selection.end
    }

    pub fn position(&self) -> &Point {
        assert_eq!(self.start(), self.end());
        self.start()
    }

    pub fn position_mut(&mut self) -> &mut Point {
        assert_eq!(self.start(), self.end());
        self.start_mut()
    }

    pub fn move_to(&mut self, y: usize, x: usize) {
        self.selection.start.y = y;
        self.selection.start.x = x;
        self.selection.end = self.selection.start;
    }

    pub fn selection(&self) -> &Range {
        &self.selection
    }

    pub fn selection_mut(&mut self) -> &mut Range {
        &mut self.selection
    }

    pub fn clear_selection(&mut self) {
        self.selection.end = self.selection.start;
    }

    pub fn is_selection(&self) -> bool {
        self.selection.start != self.selection.end
    }

    pub fn intersects_with(&self, other: &Cursor) -> bool {
        !(self.selection.end.y < other.selection.start.y
            || other.selection.end.y < self.selection.start.y
            || (self.selection.end.y == other.selection.start.y
                && self.selection.end.x < other.selection.start.x)
            || (other.selection.end.y == self.selection.start.y
                && other.selection.end.x < self.selection.start.x)
            )
    }

    pub fn contains(&self, pos: &Point) -> bool {
        self.selection.start.y <= pos.y &&  pos.y <= self.selection.end.y
        && !((pos.y == self.selection.start.y && pos.x < self.selection.start.x)
            || (pos.y == self.selection.end.y && pos.x >= self.selection.end.x))
    }

    // XXX:
    pub fn swap_start_and_end(&mut self) {
        if self.selection.end.y < self.selection.start.y
            || (self.selection.start.y == self.selection.end.y
                && self.selection.end.x < self.selection.start.x) {
            let tmp = self.selection.end;
            self.selection.end = self.selection.start;
            self.selection.start = tmp;
        }
    }

}

impl fmt::Debug for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.selection)
    }
}

pub struct Line {
    text: String,
    indices: Vec<usize>,
}

impl Line {
    pub fn new() -> Line {
        Line {
            text: String::new(),
            indices: Vec::new(),
        }
    }

    pub fn from(s: &str) -> Line {
        Line::from_string(s.to_owned())
    }

    pub fn from_string(s: String) -> Line {
        let mut line = Line {
            text: s.to_owned(),
            indices: Vec::with_capacity(s.len()),
        };

        line.update_indices();
        line
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.as_str().as_bytes()
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn at(&self, index: usize) -> char {
        debug_assert!(index < self.len());

        // This should be safe unless you forgot to self.update_indices().
        unsafe {
            if index == self.len() - 1 {
                self.text
                    .get_unchecked(self.indices[index]..)
                    .chars()
                    .next()
                    .unwrap()
            } else {
                self.text
                    .get_unchecked(self.indices[index]..self.indices[index + 1])
                    .chars()
                    .next()
                    .unwrap()
            }
        }
    }

    /// Returns the indent string in this string. It is guaranteed that the
    /// returned string contains only ASCII characters, specficically, ' ' or
    /// '\t'.
    pub fn indent(&self) -> &str {
        let mut end = 0;
        for (i, c) in self.text.char_indices() {
            if c != ' ' && c != '\t' {
                break;
            }
            end = i + 1;
        }

        &self.text[..end]
    }

    pub fn substr_from(&self, from: usize) -> &str {
        debug_assert!(from <= self.len());
        if from == self.len() {
            return "";
        }

        self.substr(from, self.len() - from)
    }

    pub fn substr(&self, from: usize, len: usize) -> &str {
        if self.is_empty() {
            return "";
        }

        let start = min(from, self.indices.len().saturating_sub(1));
        let end = from + len;
        if end < self.indices.len() {
            &self.text[self.indices[start]..self.indices[end]]
        } else {
            &self.text[self.indices[start]..]
        }
    }

    pub fn split(&self, index: usize) -> (Line, Line) {
        let prev = Line::from(&self.text[..self.indices[index]]);
        let next = Line::from(&self.text[self.indices[index]..]);
        (prev, next)
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.indices.clear();
    }

    pub fn append(&mut self, s: &str) {
        self.text += s;
        self.update_indices();
    }

    fn byte_index(&self, index: usize) -> usize {
        if self.len() == index {
            self.text.len()
        } else {
            self.indices[index]
        }
    }

    pub fn insert(&mut self, index: usize, ch: char) {
        self.text.insert(self.byte_index(index), ch);
        self.update_indices();
    }

    pub fn insert_str(&mut self, index: usize, s: &str) {
        self.text.insert_str(self.byte_index(index), s);
        self.update_indices();
    }

    pub fn remove(&mut self, index: usize) {
        debug_assert!(index < self.len());
        self.text.remove(self.byte_index(index));
        self.update_indices();
    }

    pub fn truncate(&mut self, from: usize) {
        debug_assert!(from <= self.len());
        if from == self.len() {
            return;
        }

        self.text.truncate(self.indices[from]);
        self.update_indices();
    }

    fn update_indices(&mut self) {
        self.indices.clear();
        for index in self.text.char_indices() {
            self.indices.push(index.0);
        }
    }
}

#[derive(Debug)]
pub enum Diff {
    // Contains removed char/string.
    InsertChar(Point, char),
    BackspaceChar(Point, char),
    Remove(Point, Point, String),
    Move(Vec<Cursor>),
    Stop,
}

impl Diff {
    pub fn apply(&self, lines: &mut Vec<Line>) -> Point {
        match self {
            Diff::Stop | Diff::Move(_) => unreachable!(),
            Diff::InsertChar(mut pos, ch) => {
                if *ch == '\n' {
                    let current_line = &lines[pos.y];
                    if pos.x == current_line.len() {
                        lines.insert(pos.y + 1, Line::new());
                    } else {
                        let (prev, next) = current_line.split(pos.x);
                        lines[pos.y] = prev;
                        lines.insert(pos.y + 1, next);
                    }
                    pos.y += 1;
                    pos.x = 0;
                } else {
                    lines[pos.y].insert(pos.x, *ch);
                    pos.x += 1;
                }

                pos
            }
            Diff::BackspaceChar(mut pos, _) => {
                if pos.x == 0 {
                    let tmp = lines[pos.y].as_str().to_owned();
                    let x = lines[pos.y - 1].len();
                    lines[pos.y - 1].append(&tmp);
                    lines.remove(pos.y);
                    pos.y -= 1;
                    pos.x = x;
                } else {
                    lines[pos.y].remove(pos.x - 1);
                    pos.x -= 1;
                }

                pos
            }
            Diff::Remove(start, end, _) => {
                let mut end_line_start = start.x;
                let mut end_y = end.y;

                // Remove the lines except the last line.
                debug_assert!(start.y <= end_y);
                while start.y < end_y {
                    lines[start.y].truncate(start.x);
                    let tmp = lines[start.y + 1].as_str().to_owned();
                    lines[start.y].append(&tmp);
                    lines.remove(start.y + 1);
                    end_line_start = 0;
                    end_y -= 1;
                }

                // Remove the characters in the last line.
                debug_assert!(start.y == end_y);
                for _ in 0..(end.x - end_line_start) {
                    lines[start.y].remove(start.x);
                }

                *start
            }
        }
    }

    pub fn revert(&self, lines: &mut Vec<Line>) {
        match self {
            Diff::Stop | Diff::Move(_) => unreachable!(),
            Diff::InsertChar(pos, ch) => {
                // Remove (backspace) a character.
                if *ch == '\n' {
                    if pos.y < lines.len() - 1 {
                        let tmp = lines[pos.y + 1].as_str().to_owned();
                        lines[pos.y].append(&tmp);
                        lines.remove(pos.y + 1);
                    } else {
                        lines.push(Line::new());
                    }
                } else {
                    lines[pos.y].remove(pos.x);
                }
            }
            Diff::BackspaceChar(pos, ch) => {
                // Insert the character.
                lines[pos.y].insert(pos.x - 1, *ch);
            }
            Diff::Remove(start, _, removed) => {
                // Insert the string.
                let mut y = start.y;
                let mut x = start.x;
                let rest = lines[y].substr_from(x).to_owned();
                lines[y].truncate(x);
                let mut iter = removed.lines().peekable();
                while let Some(line) = iter.next() {
                    lines[y].insert_str(x, line);
                    if iter.peek().is_some() {
                        lines.insert(y + 1, Line::new());
                    }
                    y += 1;
                    x = 0;
                }

                if removed.ends_with('\n') {
                    lines.insert(y, Line::new());
                    y += 1;
                }

                let len = lines[y - 1].len();
                lines[y - 1].insert_str(len, &rest);
            }
        }
    }
}
