use std::fs;
use crate::screen::Position;

pub struct Line<'a> {
    line: Option<&'a str>,
}

impl<'a> Iterator for Line<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        self.line.take()
    }
}

pub struct Buffer {
    name: String,
    lines: Vec<String>,
    modified: bool,
}

impl Buffer {
    pub fn new(name: &str) -> Buffer {
        let mut lines = Vec::with_capacity(1024);
        lines.push(String::new());
        Buffer { name: name.to_owned(), lines, modified: false }
    }

    pub fn from_file(name: &str, handle: &fs::File) -> std::io::Result<Buffer> {
        use std::io::BufRead;

        let reader = std::io::BufReader::new(handle.try_clone()?);
        let mut lines = Vec::with_capacity(1024);
        for line in reader.lines() {
            lines.push(line?);
        }

        if lines.is_empty() {
            lines.push(String::new());
        }

        Ok(Buffer { name: name.to_owned(), lines, modified: false })
    }

    pub fn line_at<'a>(&'a self, y: usize) -> Line<'a> {
        let line = Some(self.lines[y].as_str());
        Line { line }
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
            let after_cursor = self.lines[pos.line][pos.column..].to_owned();
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
            let copied_str = self.lines[pos.line].to_owned();
            self.lines[pos.line - 1].push_str(&copied_str);
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
                let s = self.lines[pos.line + 1].to_owned();
                self.lines.remove(pos.line + 1);
                s
            } else {
                String::new()
            };
            self.lines[pos.line].push_str(&trailing);
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
