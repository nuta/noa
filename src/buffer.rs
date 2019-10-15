use std::fs;
use std::ops;
use crate::screen::Position;

/// A `String` with cached UTF-8 character byte indices.
#[derive(Clone)]
pub struct IString {
    text: String,
    indices: Vec<usize>,
}

impl IString {
    pub fn new() -> IString {
        IString {
            text: String::new(),
            indices: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> IString {
        IString {
            text: String::with_capacity(capacity),
            indices: Vec::with_capacity(capacity),
        }
    }

    pub fn from_string(string: String) -> IString {
        let len = string.len();
        let mut new_string = IString {
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
        self.text.as_str()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    pub fn at(&self, index: usize) -> Option<char> {
        if index < self.indices.len() {
            self.text.as_str()[self.indices[index]..].chars().next()
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.indices.clear();
    }

    pub fn insert(&mut self, index: usize, ch: char) {
        self.text.insert(self.offset_at(index), ch);
        self.update_indices();
    }

    pub fn push(&mut self, ch: char) {
        self.text.push(ch);
        self.update_indices();
    }

    pub fn push_str(&mut self, other: &str) {
        self.text.push_str(other);
        self.update_indices();
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
        for (i, _) in self.text.char_indices() {
            self.indices.push(i);
        }
    }
}

impl ops::Index<ops::RangeFrom<usize>> for IString {
    type Output = str;
    fn index(&self, index: ops::RangeFrom<usize>) -> &str {
        if index.start == self.len() {
            ""
        } else {
            &self.text[self.indices[index.start]..]
        }
    }
}

impl ops::Index<ops::Range<usize>> for IString {
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

pub struct Lines<'a>(std::slice::Iter<'a, IString>);

impl<'a> Iterator for Lines<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|s| s.as_str())
    }
}

pub struct Buffer {
    name: String,
    lines: Vec<IString>,
    modified: bool,
}

impl Buffer {
    pub fn new(name: &str) -> Buffer {
        let mut lines = Vec::with_capacity(1024);
        lines.push(IString::with_capacity(128));
        Buffer { name: name.to_owned(), lines, modified: false }
    }

    pub fn from_file(name: &str, handle: &fs::File) -> std::io::Result<Buffer> {
        use std::io::BufRead;

        let reader = std::io::BufReader::new(handle.try_clone()?);
        let mut lines = Vec::with_capacity(1024);
        // TODO: Support CRLF.
        for line in reader.lines() {
            let mut s = IString::from_string(line?);
            s.push('\n');
            lines.push(s);
        }

        if lines.is_empty() {
            lines.push(IString::new());
        }

        Ok(Buffer { name: name.to_owned(), lines, modified: false })
    }

    pub fn lines_from<'a>(&'a self, from: usize) -> Lines<'a> {
        Lines(self.lines[from..].iter())
    }

    pub fn line_at(&self, line: usize) -> &IString {
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
                IString::from_string(self.lines[pos.line][pos.column..].to_owned());
            self.lines[pos.line].truncate(pos.column);
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
            let prev_line_len = self.lines[pos.line - 1].len();
            let copied_str = self.lines[pos.line].clone();
            self.lines[pos.line - 1].push_str(copied_str.as_str());
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
                IString::new()
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
            handle.write_all(line.as_bytes())?;
            // FIXME: Don't append the newline at EOF.
            handle.write(b"\n")?;
        }

        self.modified = false;
        Ok(())
    }
}
