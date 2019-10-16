use std::fs;
use std::ops;
use crate::screen::Position;

/// A `String` with cached UTF-8 character byte indices.
#[derive(Clone)]
pub struct Line {
    /// A UTF-8 string.
    text: String,
    /// The byte offsets of each characters.
    indices: Vec<usize>,
}

impl Line {
    pub fn new() -> Line {
        Line {
            text: String::new(),
            indices: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Line {
        Line {
            text: String::with_capacity(capacity),
            indices: Vec::with_capacity(capacity),
        }
    }

    pub fn from_string(string: String) -> Line {
        let len = string.len();
        let mut new_string = Line {
            text: string,
            indices: Vec::with_capacity(len),
        };

        new_string.update_indices();
        new_string
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn as_str(&self) -> &str {
        &self.text[..self.len()]
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.as_str().as_bytes()
    }

    pub fn as_str_with_newline(&self) -> &str {
        &self.text.as_str()
    }

    pub fn as_bytes_with_newline(&self) -> &[u8] {
        self.as_str_with_newline().as_bytes()
    }

    pub fn at(&self, index: usize) -> Option<char> {
        if index < self.indices.len() {
            self.as_str()[self.indices[index]..].chars().next()
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.indices.clear();
    }

    pub fn insert(&mut self, index: usize, ch: char) {
        debug_assert!(ch != '\n' && ch != '\r');
        self.text.insert(self.offset_at(index), ch);
        self.update_indices();
    }

    pub fn push(&mut self, ch: char) {
        debug_assert!(ch != '\n' && ch != '\r');
        self.text.push(ch);
        self.update_indices();
    }

    pub fn push_str(&mut self, other: &str) {
        debug_assert!(!other.contains('\r') && !other.contains('\n'));
        self.text.push_str(other);
        self.update_indices();
    }

    pub fn set_newline(&mut self, newline: &str) {
        self.remove_newline();
        self.text.push_str(newline);
        self.update_indices();
    }

    pub fn remove_newline(&mut self) {
        while self.text.len() > 0 {
            let ch = self.text.as_bytes()[self.text.len() - 1];
            if ch != b'\r' && ch != b'\n' {
                break;
            }

            self.text.pop();
        }
    }

    pub fn remove(&mut self, index: usize) {
        self.text.remove(self.offset_at(index));
        self.update_indices();
    }

    pub fn truncate(&mut self, from: usize) {
        self.text.truncate(self.offset_at(from));
        self.update_indices();
    }

    fn offset_at(&self, index: usize) -> usize {
        if index == self.indices.len() {
            self.text.len()
        } else {
            self.indices[index]
        }
    }

    fn update_indices(&mut self) {
        self.indices.clear();
        let iter = self.text
            .trim_end_matches(|c: char| c == '\n' || c == '\r')
            .char_indices();
        for (i, _) in iter {
            self.indices.push(i);
        }
    }
}

impl ops::Index<ops::RangeFrom<usize>> for Line {
    type Output = str;
    fn index(&self, index: ops::RangeFrom<usize>) -> &str {
        if index.start == self.len() {
            ""
        } else {
            &self.text[self.indices[index.start]..]
        }
    }
}

impl ops::Index<ops::Range<usize>> for Line {
    type Output = str;
    fn index(&self, index: ops::Range<usize>) -> &str {
        debug_assert!(index.end <= self.len());
        let end = if index.end == self.len() {
            self.as_str().len()
        } else {
            self.indices[index.end]
        };

        &self.text[self.indices[index.start]..end]
    }
}

pub struct Buffer {
    name: String,
    lines: Vec<Line>,
    modified: bool,
    newline: &'static str,
}

impl Buffer {
    pub fn new(name: &str) -> Buffer {
        let mut lines = Vec::with_capacity(1024);
        lines.push(Line::with_capacity(128));
        Buffer { name: name.to_owned(), lines, modified: false, newline: "\n" }
    }

    pub fn from_file(name: &str, handle: &fs::File) -> std::io::Result<Buffer> {
        use std::io::BufRead;

        let mut reader = std::io::BufReader::new(handle.try_clone()?);
        let mut lines = Vec::with_capacity(1024);
        loop {
            let mut buf = String::with_capacity(128);
            match reader.read_line(&mut buf) {
                // EOF.
                Ok(0) => break,
                Ok(_) => lines.push(Line::from_string(buf)),
                Err(err) => return Err(err),
            };
        }

        if lines.is_empty() {
            lines.push(Line::new());
        }

        Ok(Buffer {
            name: name.to_owned(),
            lines,
            modified: false,
            newline: "\n"
        })
    }

    pub fn lines_from<'a>(&'a self, from: usize) -> impl Iterator<Item=&'a Line> {
        self.lines[from..].iter()
    }

    pub fn line_at(&self, line: usize) -> &Line {
        assert!(line < self.num_lines());
        &self.lines[line]
    }

    pub fn line_len_at(&self, y: usize) -> usize {
        self.lines[y].len()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    pub fn insert(&mut self, pos: &Position, ch: char) {
        if ch == '\n' {
            let after_cursor =
                Line::from_string(self.lines[pos.line][pos.column..].to_owned());
            self.lines[pos.line].truncate(pos.column);
            self.lines[pos.line].set_newline(self.newline);
            self.lines.insert(pos.line + 1, after_cursor);
        } else {
            self.lines[pos.line].insert(pos.column, ch);
        }

        self.modified = true;
    }

    /// Returns the length of the previous line before removing a character if
    /// a newline is deleted.
    pub fn backspace(&mut self, pos: &Position) -> Option<usize> {
        self.modified = true;
        if pos.column == 0 {
            // FIXME: Avoid temporary copy.
            let copied_str = self.lines[pos.line].clone();
            let prev_line = &mut self.lines[pos.line - 1];
            let prev_line_len = prev_line.len();
            prev_line.remove_newline();
            prev_line.push_str(copied_str.as_str());
            self.lines.remove(pos.line);
            Some(prev_line_len)
        } else {
            self.lines[pos.line].remove(pos.column - 1);
            None
        }
    }

    pub fn delete(&mut self, pos: &Position) {
        debug_assert!(pos.column <= self.lines[pos.line].len());
        if pos.column == self.lines[pos.line].len() {
            // Delete at the end of the line: remove a newline.
            // FIXME: Avoid temporary copy.
            let trailing = if pos.line + 1 < self.num_lines() {
                let s = self.lines[pos.line + 1].clone();
                self.lines.remove(pos.line + 1);
                s
            } else {
                Line::new()
            };
            self.lines[pos.line].push_str(trailing.as_str());
        } else if pos.column < self.lines[pos.line].len() {
            self.lines[pos.line].remove(pos.column);
        }

        self.modified = true;
    }

    pub fn write_to_file(&mut self, handle: &mut fs::File) -> std::io::Result<()> {
        use std::io::Write;
        for line in &self.lines {
            handle.write_all(line.as_bytes_with_newline())?;
        }

        self.modified = false;
        Ok(())
    }
}
