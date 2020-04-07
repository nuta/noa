use crate::editorconfig::{EditorConfig, EndOfLine, IndentStyle};
use std::cmp::min;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
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

    pub fn remove(&mut self, index: usize) {
        debug_assert!(index < self.len());
        self.text.remove(self.byte_index(index));
        self.update_indices();
    }

    pub fn truncate(&mut self, from: usize) {
        debug_assert!(from < self.len());
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

/// Normalizes a relative path. Unlike std::fs::cannonicalize, it does not
/// follow symbolic links and does not return an error even if the file does not
/// exists.
fn abspath(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut segments: Vec<String> = if path.starts_with("/") {
        vec!["".to_owned()]
    } else {
        std::env::current_dir().unwrap()
          .to_str().unwrap()
          .split("/").map(|s| s.to_owned()).collect()
    };

    for c in path.components() {
        match c {
            Component::ParentDir => {
                segments.pop();
                if segments.is_empty() {
                    segments.push("".to_owned());
                }
            }
            Component::RootDir | Component::CurDir => { /* Nothing to do. */ }
            Component::Normal(s) => {
                segments.push(s.to_str().unwrap().to_owned());
            }
            _ => { unimplemented!(); }
        }
    }

    Path::new(&segments.join("/")).to_path_buf()
}

#[derive(Debug)]
pub enum Action {
    Insert {
        pos: Point,
        num: usize,
        text: String,
    },
    Remove {
        pos: Point,
        num: usize,
        text: String,
    },
}

pub struct Buffer {
    display_name: String,
    cursor: Point,
    top_left: Point,
    file: Option<PathBuf>,
    backup_file: Option<PathBuf>,
    original_hash: u64,
    modified: bool,
    lines: Vec<Line>,
    config: EditorConfig,

    uncommitted_actions: Vec<Action>,
    undo_stack: Vec<Action>,
    redo_stack: Vec<Action>,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            display_name: "".to_owned(),
            cursor: Point { x: 0, y: 0 },
            top_left: Point { x: 0, y: 0 },
            file: None,
            backup_file: None,
            original_hash: 0,
            modified: false,
            lines: vec![Line::new()],
            uncommitted_actions: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            config: EditorConfig::default(),
        }
    }

    pub fn open_file(path: &Path) -> Result<Buffer, std::io::Error> {
        use std::hash::Hasher;
        use std::io::BufRead;

        let mut buffer = Buffer::new();
        let mut hasher = fxhash::FxHasher32::default();
        if path.exists() {
            let file = std::fs::File::open(path)?;
            let reader = std::io::BufReader::new(file);
            buffer.lines.clear();
            for line_string in reader.lines() {
                let line = Line::from_string(line_string?);
                hasher.write(line.as_bytes());
                hasher.write(b"\n");
                buffer.lines.push(line);
            }
        }

        let last_line = Line::new();
        hasher.write(last_line.as_bytes());
        hasher.write(b"\n");
        buffer.lines.push(last_line);

        let backup_dir = dirs::home_dir()
            .unwrap().join(".noa").join("backup");
        if !backup_dir.exists() {
            std::fs::create_dir_all(&backup_dir)?;
        }

        // "/Users/seiya/foo.txt" -> "Users.seiya.foo.txt"
        let normalized_path = abspath(path);
        let filename = normalized_path
            .strip_prefix("/")
            .unwrap_or(path)
            .to_str()
            .unwrap()
            .replace('/', ".");

        buffer.config = EditorConfig::resolve(&normalized_path);
        buffer.display_name = normalized_path
            .file_name()
            .map(|s| s.to_str().unwrap())
            .unwrap_or("invalid filename")
            .to_owned();
        buffer.file = Some(normalized_path);
        buffer.backup_file = Some(backup_dir.join(filename).to_path_buf());
        buffer.original_hash = hasher.finish();

        Ok(buffer)
    }

    fn write_into_file(&self, path: &Path) -> Result<(), std::io::Error> {
        let mut file = std::fs::File::create(path)?;
        let newline = match self.config.end_of_line {
            EndOfLine::Cr => "\r",
            EndOfLine::Lf => "\n",
            EndOfLine::CrLf => "\r\n",
        };

        let mut iter = self.lines.iter().peekable();
        while let Some(line) = iter.next() {
            if iter.peek().is_none() && line.is_empty() {
                // Ignore the last line if it's empty.
                break;
            }

            use std::io::Write;
            file.write(line.as_bytes()).ok();
            file.write(newline.as_bytes()).ok();
        }

        Ok(())
    }

    pub fn backup_path(&self) -> Option<&PathBuf> {
        self.backup_file.as_ref()
    }

    pub fn backup(&self) -> Result<(), std::io::Error> {
        if let Some(backup_file) = &self.backup_file {
            self.write_into_file(backup_file)?;
        }

        Ok(())
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if let Some(path) = &self.file {
            trace!("saving...");
            self.write_into_file(path)?;
            self.modified = false;
            self.original_hash = self.hash();

            // We no longer need the backup file. Remove it.
            let backup = self.backup_path().unwrap();
            if backup.exists() {
                std::fs::remove_file(backup).ok();
            }
        }

        Ok(())
    }

    pub fn file(&self) -> Option<&PathBuf> {
        self.file.as_ref()
    }

    pub fn hash(&self) -> u64 {
        use std::hash::Hasher;

        let mut hasher = fxhash::FxHasher32::default();
        for line in &self.lines {
            hasher.write(line.as_bytes());
            hasher.write(b"\n");
        }

        hasher.finish()
    }

    pub fn modified(&mut self) -> bool {
        if !self.modified || self.file.is_none() {
            return false;
        }

        if self.hash() == self.original_hash {
            self.modified = false;
            false
        } else {
            true
        }
    }

    pub fn set_display_name(&mut self, name: String) {
        self.display_name = name;
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn cursor(&self) -> &Point {
        &self.cursor
    }

    pub fn top_left(&self) -> &Point {
        &self.top_left
    }

    pub fn config(&self) -> &EditorConfig {
        &self.config
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        for line in self.lines() {
            s += line.as_str();
        }
        s
    }

    pub fn lines(&self) -> std::slice::Iter<Line> {
        self.lines.iter()
    }

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    pub fn insert(&mut self, ch: char) {
        self.push_action(Action::Insert {
            pos: self.cursor,
            text: ch.to_string(),
            num: 1,
        });
        self.modified = true;
        if ch == '\n' {
            let current_line = &self.lines[self.cursor.y];
            if self.cursor.x == current_line.len() {
                self.lines.insert(self.cursor.y + 1, Line::new());
            } else {
                let (prev, next) = current_line.split(self.cursor.x);
                self.lines[self.cursor.y] = prev;
                self.lines.insert(self.cursor.y + 1, next);
            }
            self.cursor.y += 1;
            self.cursor.x = 0;

            // Auto indentation.
            self.tab(true);
        } else {
            self.lines[self.cursor.y].insert(self.cursor.x, ch);
            self.cursor.x += 1;
        }
    }

    pub fn backspace(&mut self) {
        self.modified = true;
        if self.cursor.y == 0 && self.cursor.x == 0 {
            /* Do nothing. */
        } else if self.cursor.x == 0 {
            let tmp = self.lines[self.cursor.y].as_str().to_owned();
            let x = self.lines[self.cursor.y - 1].len();
            self.lines[self.cursor.y - 1].append(&tmp);
            self.lines.remove(self.cursor.y);
            self.cursor.y -= 1;
            self.cursor.x = x;
        } else {
            let indent_len = self.lines[self.cursor.y].indent().len();
            if self.cursor.x <= indent_len {
                // Decrease the indentation level.
                let mut num_remove = self.cursor.x % self.config.indent_size;
                if num_remove == 0 {
                    num_remove = self.config.indent_size;
                }

                for _ in 0..num_remove {
                    self.lines[self.cursor.y].remove(self.cursor.x - 1);
                    self.cursor.x -= 1;
                }
            } else {
                // Remove a character.
                self.push_action(Action::Remove {
                    pos: Point {
                        y: self.cursor.y,
                        x: self.cursor.x - 1,
                    },
                    num: 1,
                    text: self.lines[self.cursor.y]
                        .at(self.cursor.x - 1).to_string()
                });
                self.lines[self.cursor.y].remove(self.cursor.x - 1);
                self.cursor.x -= 1;
            }
        }
    }

    pub fn delete(&mut self) {
        self.modified = true;
        let at_eol = self.cursor.x == self.lines[self.cursor.y].len();
        if self.cursor.y == self.num_lines() - 1 && at_eol {
            /* Do nothing. */
        } else if at_eol {
            let tmp = self.lines[self.cursor.y + 1].as_str().to_owned();
            self.lines[self.cursor.y].append(&tmp);
            self.lines.remove(self.cursor.y + 1);
        } else {
            self.lines[self.cursor.y].remove(self.cursor.x);
        }
    }

    pub fn truncate(&mut self) {
        self.modified = true;
        if self.lines[self.cursor.y].is_empty() {
            if self.cursor.y < self.lines.len() - 1 {
                self.lines.remove(self.cursor.y);
            }
        } else if self.cursor.x == self.lines[self.cursor.y].len() {
            // Remove the newline.
            self.delete();
        } else {
            self.lines[self.cursor.y].truncate(self.cursor.x);
        }
    }

    pub fn tab(&mut self, after_newline: bool) {
        self.modified = true;
        match self.config.indent_style {
            IndentStyle::Tab => self.insert('\t'),
            IndentStyle::Space => {
                let indent_size = self.config.indent_size;
                let indent_len = if after_newline || self.lines[self.cursor.y].is_empty() {
                    if after_newline && self.cursor.y > 0 {
                        // Inherit the previous indent.
                        self.lines[self.cursor.y - 1].indent().len()
                    } else if !after_newline {
                        indent_size
                    } else {
                        0
                    }
                } else {
                    // Increase the indentation level.
                    indent_size
                };

                if indent_len > 0 {
                    for _ in 0..(indent_len - (self.cursor.x % indent_len)) {
                        self.insert(' ');
                    }
                }
            }
        }
    }

    pub fn move_by(&mut self, y_diff: isize, x_diff: isize) {
        debug_assert!(y_diff.abs() <= 1 && x_diff.abs() <= 1);

        if x_diff < 0 {
            if (self.cursor.x as isize) < x_diff.abs() && self.cursor.y > 0 {
                // Move to the previous line.
                self.cursor.y -= 1;
                self.cursor.x = self.lines[self.cursor.y].len();
            } else if self.cursor.x >= x_diff.abs() as usize {
                self.cursor.x -= x_diff.abs() as usize;
            }
        } else if x_diff > 0 {
            if self.cursor.x == self.lines[self.cursor.y].len() {
                if self.cursor.y < self.num_lines() - 1 {
                    // Move to the next line.
                    self.cursor.y += 1;
                    self.cursor.x = 0;
                }
            } else {
                self.cursor.x += x_diff.abs() as usize;
            }
        }

        if y_diff < 0 {
            self.cursor.y = self.cursor.y.saturating_sub(y_diff.abs() as usize);
        } else {
            self.cursor.y += y_diff.abs() as usize;
        }

        self.cursor.y = min(self.cursor.y, self.num_lines() - 1);
        self.cursor.x = min(self.cursor.x, self.lines[self.cursor.y].len());
    }

    pub fn adjust_top_left(&mut self, height: usize, width: usize) {
        // Scroll Up.
        if self.cursor.y < self.top_left.y {
            self.top_left.y = self.cursor.y;
        }

        // Scroll Down.
        if self.cursor.y >= self.top_left.y + height {
            self.top_left.y = self.cursor.y - height + 1;
        }

        // Scroll Right.
        if self.cursor.x >= self.top_left.x + width {
            self.top_left.x = self.cursor.x - width + 1;
        }

        // Scroll Left.
        if self.cursor.x < self.top_left.x {
            self.top_left.x = self.cursor.x;
        }
    }

    pub fn scroll_up(&mut self, height: usize) {
        let relative_y = self.cursor.y - self.top_left.y;
        self.top_left.y = self.top_left.y.saturating_sub(height);
        self.cursor.y = self.top_left.y + relative_y;
    }

    pub fn scroll_down(&mut self, height: usize) {
        if self.num_lines() >= self.top_left.y + height {
            let relative_y = self.cursor.y - self.top_left.y;
            self.top_left.y = min(self.num_lines() - 1, self.top_left.y + height);
            self.cursor.y = min(self.num_lines() - 1, self.top_left.y + relative_y);
        }
    }

    pub fn move_to_begin(&mut self) {
        let old = self.cursor.x;
        self.cursor.x = 0;
        while self.cursor.x < self.lines[self.cursor.y].len() {
            if !self.lines[self.cursor.y].at(self.cursor.x).is_whitespace() {
                break;
            }

            self.cursor.x += 1;
        }

        if self.cursor.x == old {
            self.cursor.x = 0;
        }
    }

    pub fn move_to_end(&mut self) {
        self.cursor.x = self.lines[self.cursor.y].len();
    }

    pub fn undo(&mut self) {
        self.commit_actions();
        if let Some(action) = self.undo_stack.pop() {
            self.undo_action(&action);
            self.uncommitted_actions.clear();
            self.redo_stack.push(action);
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.redo_stack.pop() {
            self.redo_action(&action);
            self.uncommitted_actions.clear();
            self.undo_stack.push(action);
        }
    }

    /// Reverts the changes by `action`.
    pub fn undo_action(&mut self, action: &Action) {
        match action {
            Action::Insert { pos, num, .. } => {
                self.cursor.y = pos.y;
                self.cursor.x = pos.x;
                for _ in 0..*num {
                    self.delete();
                }
            }
            Action::Remove { pos, text, .. } => {
                self.cursor.y = pos.y;
                self.cursor.x = pos.x;
                for ch in text.chars() {
                    self.insert(ch);
                }
            }
        }
    }

    /// Applies `action`.
    pub fn redo_action(&mut self, action: &Action) {
        match action {
            Action::Insert { pos, text, .. } => {
                self.cursor.y = pos.y;
                self.cursor.x = pos.x;
                for ch in text.chars() {
                    self.insert(ch);
                }
            }
            Action::Remove { pos, num, .. } => {
                self.cursor.y = pos.y;
                self.cursor.x = pos.x;
                for _ in 0..*num {
                    self.delete();
                }
            }
        }
    }

    pub fn commit_actions(&mut self) {
        let mut iter = self.uncommitted_actions.drain(..).peekable();
        if iter.peek().is_some() {
            self.redo_stack.clear();
        }

        // Merge actions.
        while let Some(mut action) = iter.next() {
            while let Some(next_action) = iter.peek() {
                match (&mut action, next_action) {
                    (Action::Insert { pos, num, text },
                     Action::Insert { pos: pos2, num: num2, text: text2 })
                        if pos.y == pos2.y && pos.x + *num == pos2.x
                            && !text.contains('\n') && !text.contains('\n') => {
                            text.push_str(text2);
                            *num += num2;
                            iter.next();
                        }
                    (Action::Remove { pos, num, text },
                     Action::Remove { pos: pos2, num: num2, text: text2 })
                        if pos.y == pos2.y && pos.x == pos2.x + 1
                            && !text.contains('\n') && !text.contains('\n') => {
                            text.insert_str(0, text2);
                            *num += num2;
                            pos.x = pos2.x;
                            iter.next();
                        }
                    (_, _) => {
                        break;
                    }
                }
            }

            self.undo_stack.push(action);
        }
    }

    pub fn push_action(&mut self, action: Action) {
        self.uncommitted_actions.push(action);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_undo() {
        let mut b = Buffer::new();
        b.insert('A');
        b.insert('B');
        b.commit_actions();

        assert_eq!(b.text(), "AB");
        b.undo();
        assert_eq!(b.text(), "");
        b.redo();
        assert_eq!(b.text(), "AB");

        b.insert('C');
        b.commit_actions();
        assert_eq!(b.text(), "ABC");

        b.undo();
        assert_eq!(b.text(), "AB");
        b.insert('D');
        assert_eq!(b.text(), "ABD");
        b.undo(); // AB
        b.undo(); //
        b.redo(); // AB
        b.redo(); // ABD
        assert_eq!(b.text(), "ABD");
    }
}
