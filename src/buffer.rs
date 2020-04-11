use crate::editorconfig::{EditorConfig, EndOfLine, IndentStyle};
use std::cmp::min;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

impl Point {
    pub const fn new(x: usize, y: usize) -> Point {
        Point { x, y }
    }
}

impl fmt::Debug for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
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

#[derive(Debug)]
enum Command {
    Insert(char),
    Backspace,
    Delete,
    Truncate,
    Tab(bool),
    MoveBy {
        y_diff: isize,
        x_diff: isize,
    },
    ScrollUp(usize),
    ScrollDown(usize),
    MoveToBegin,
    MoveToEnd,
}

#[derive(Debug)]
enum Selection {
    Left,
    Right,
}

pub struct Buffer {
    display_name: String,
    cursors: Vec<Cursor>,
    top_left: Point,
    file: Option<PathBuf>,
    backup_file: Option<PathBuf>,
    original_hash: u64,
    modified: bool,
    lines: Vec<Line>,
    config: EditorConfig,
    selection: Option<Selection>,

    uncommitted_actions: Vec<Action>,
    undo_stack: Vec<Action>,
    redo_stack: Vec<Action>,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            display_name: "".to_owned(),
            cursors: vec![Cursor::new(Point::new(0, 0))],
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
            selection: None,
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

    pub fn cursors(&self) -> &[Cursor] {
        &self.cursors
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

    pub fn line_at(&self, lineno: usize) -> &Line {
        &self.lines[lineno]
    }

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    fn process_command(&mut self, cmd: Command) {
        let mut cursors = self.cursors.clone();
        let mut new_cursors = Vec::with_capacity(cursors.len());
        let mut end_selection = true;
        for (i, cursor) in cursors.iter_mut().enumerate() {
            // Remove out-of-range cursors and duplicated ones.
            /* TODO:
            if pos.y >= self.num_lines()
                || pos.x > self.line_at(pos.y).len()
                || (i + 1 < self.cursors.len() &&
                    self.cursors.iter().skip(i + 1).any(|&c| c.intersects_with(cursor))) {
                continue;
            }
            */

            match cmd {
                Command::Insert(ch) => {
                    self.remove_selection(cursor);
                    self.do_insert(cursor.position_mut(), ch);
                }
                Command::Tab(after_newline) => {
                    self.remove_selection(cursor);
                    self.do_tab(cursor.start_mut(), after_newline);
                }
                Command::Backspace => {
                    if cursor.is_selection() {
                        self.remove_selection(cursor);
                    } else {
                        self.do_backspace(cursor.start_mut());
                    }
                },
                Command::Delete => {
                    if cursor.is_selection() {
                        self.remove_selection(cursor);
                    } else {
                        self.do_delete(cursor.start_mut());
                    }
                }
                Command::Truncate => {
                    if cursor.is_selection() {
                        self.remove_selection(cursor);
                    } else {
                        self.do_truncate(cursor.start_mut(), false);
                    }
                }
                Command::MoveBy { y_diff, x_diff } => {
                    if self.selection.is_some() && cursor.start() == cursor.end() {
                        if x_diff < 0 || y_diff < 0 {
                            self.selection = Some(Selection::Left);
                        } else {
                            self.selection = Some(Selection::Right);
                        }
                    }

                    match &mut self.selection {
                        Some(direction) => {
                            let pos = match direction {
                                Selection::Left => cursor.start_mut(),
                                Selection::Right => cursor.end_mut(),
                            };

                            self.do_move_by(pos, y_diff, x_diff);
                            end_selection = false;
                            cursor.swap_start_and_end();
                        }
                        None => {
                            cursor.clear_selection();
                            self.do_move_by(cursor.position_mut(), y_diff, x_diff);
                        }
                    }

                    info!("cursor={:?} {:?}", cursor, self.selection);
                }
                Command::ScrollUp(height) => {
                    cursor.clear_selection();
                    self.do_scroll_up(cursor.start_mut(), height);
                }
                Command::ScrollDown(height) => {
                    cursor.clear_selection();
                    self.do_scroll_down(cursor.start_mut(), height);
                }
                Command::MoveToBegin => {
                    cursor.clear_selection();
                    self.do_move_to_begin(cursor.start_mut());
                }
                Command::MoveToEnd => {
                    cursor.clear_selection();
                    self.do_move_to_end(cursor.start_mut());
                }
            }
            new_cursors.push(cursor.clone());
        }

        if end_selection {
            self.end_selection();
        }

        self.cursors = new_cursors;
    }

    pub fn start_selection(&mut self) {
        self.selection = Some(Selection::Right);
    }

    pub fn end_selection(&mut self) {
        self.selection = None;
        for cursor in &mut self.cursors {
            cursor.clear_selection();
        }
    }

    pub fn remove_selection(&mut self, cursor: &mut Cursor) {
        // A example text:
        //
        //    start
        //    V
        // ...123
        // abcdef
        // ghi...
        //    ^
        //    end

        let start = cursor.start();
        let end = cursor.end();
        let mut start_x = start.x;
        let mut end_y: usize = end.y;

        // Remove the lines except the last line (i.e. "123", "abc...").
        debug_assert!(start.y <= end_y);
        while start.y < end_y {
            trace!("start: {:?}, end_y: {}", start, end_y);
            self.do_truncate(start, true);
            start_x = 0;
            end_y -= 1;
        }

        // Remove the characters in the last line (i.e. "ghi").
        debug_assert!(start.y == end_y);
        for _ in 0..(end.x - start_x) {
            self.do_delete(start);
        }

        cursor.selection.end = *start;
    }

    pub fn insert(&mut self, ch: char) {
        self.process_command(Command::Insert(ch));
    }

    pub fn backspace(&mut self) {
        self.process_command(Command::Backspace);
    }

    pub fn delete(&mut self) {
        self.process_command(Command::Delete);
    }

    pub fn truncate(&mut self) {
        self.process_command(Command::Truncate);
    }

    pub fn tab(&mut self, after_newline: bool) {
        self.process_command(Command::Tab(after_newline));
    }

    pub fn move_by(&mut self, y_diff: isize, x_diff: isize) {
        self.process_command(Command::MoveBy { y_diff, x_diff });
    }

    pub fn scroll_up(&mut self, height: usize) {
        self.process_command(Command::ScrollUp(height));
    }

    pub fn scroll_down(&mut self, height: usize) {
        self.process_command(Command::ScrollDown(height));
    }

    pub fn move_to_begin(&mut self) {
        self.process_command(Command::MoveToBegin);
    }

    pub fn move_to_end(&mut self) {
        self.process_command(Command::MoveToEnd);
    }

    fn do_insert(&mut self, pos: &mut Point, ch: char) {
        /*
        self.push_action(Action::Insert {
            pos: *cursor,
            text: ch.to_string(),
            num: 1,
        });
        */
        self.modified = true;
        if ch == '\n' {
            let current_line = &self.lines[pos.y];
            if pos.x == current_line.len() {
                self.lines.insert(pos.y + 1, Line::new());
            } else {
                let (prev, next) = current_line.split(pos.x);
                self.lines[pos.y] = prev;
                self.lines.insert(pos.y + 1, next);
            }
            pos.y += 1;
            pos.x = 0;

            // Auto indentation.
            self.do_tab(pos, true);
        } else {
            self.lines[pos.y].insert(pos.x, ch);
            pos.x += 1;
        }
    }

    pub fn do_backspace(&mut self, pos: &mut Point) {
        self.modified = true;
        if pos.y == 0 && pos.x == 0 {
            /* Do nothing. */
        } else if pos.x == 0 {
            let tmp = self.lines[pos.y].as_str().to_owned();
            let x = self.lines[pos.y - 1].len();
            self.lines[pos.y - 1].append(&tmp);
            self.lines.remove(pos.y);
            pos.y -= 1;
            pos.x = x;
        } else {
            let indent_len = self.lines[pos.y].indent().len();
            if pos.x <= indent_len {
                // Decrease the indentation level.
                let mut num_remove = pos.x % self.config.indent_size;
                if num_remove == 0 {
                    num_remove = self.config.indent_size;
                }

                for _ in 0..num_remove {
                    self.lines[pos.y].remove(pos.x - 1);
                    pos.x -= 1;
                }
            } else {
                // Remove a character.
                self.push_action(Action::Remove {
                    pos: Point {
                        y: pos.y,
                        x: pos.x - 1,
                    },
                    num: 1,
                    text: self.lines[pos.y]
                        .at(pos.x - 1).to_string()
                });
                self.lines[pos.y].remove(pos.x - 1);
                pos.x -= 1;
            }
        }
    }

    pub fn do_delete(&mut self, pos: &Point) {
        self.modified = true;
        let at_eol = pos.x == self.lines[pos.y].len();
        if pos.y == self.num_lines() - 1 && at_eol {
            /* Do nothing. */
        } else if at_eol {
            let tmp = self.lines[pos.y + 1].as_str().to_owned();
            self.lines[pos.y].append(&tmp);
            self.lines.remove(pos.y + 1);
        } else {
            self.lines[pos.y].remove(pos.x);
        }
    }

    pub fn do_truncate(&mut self, pos: &Point, remove_newline: bool) {
        self.modified = true;
        if self.lines[pos.y].is_empty() {
            if pos.y < self.lines.len() - 1 {
                self.lines.remove(pos.y);
            }
        } else if pos.x == self.lines[pos.y].len() {
            // Remove the newline.
            self.do_delete(pos);
        } else {
            self.lines[pos.y].truncate(pos.x);
            if remove_newline {
                self.do_delete(pos);
            }
        }
    }

    pub fn do_tab(&mut self, pos: &mut Point, after_newline: bool) {
        self.modified = true;
        match self.config.indent_style {
            IndentStyle::Tab => self.do_insert(pos, '\t'),
            IndentStyle::Space => {
                let indent_size = self.config.indent_size;
                let indent_len = if after_newline || self.lines[pos.y].is_empty() {
                    if after_newline && pos.y > 0 {
                        // Inherit the previous indent.
                        self.lines[pos.y - 1].indent().len()
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
                    for _ in 0..(indent_len - (pos.x % indent_len)) {
                        self.do_insert(pos, ' ');
                    }
                }
            }
        }
    }

    pub fn do_move_by(&mut self, pos: &mut Point, y_diff: isize, x_diff: isize) {
        debug_assert!(y_diff.abs() <= 1 && x_diff.abs() <= 1);

        if x_diff < 0 {
            if (pos.x as isize) < x_diff.abs() && pos.y > 0 {
                // Move to the previous line.
                pos.y -= 1;
                pos.x = self.lines[pos.y].len();
            } else if pos.x >= x_diff.abs() as usize {
                pos.x -= x_diff.abs() as usize;
            }
        } else if x_diff > 0 {
            if pos.x == self.lines[pos.y].len() {
                if pos.y < self.num_lines() - 1 {
                    // Move to the next line.
                    pos.y += 1;
                    pos.x = 0;
                }
            } else {
                pos.x += x_diff.abs() as usize;
            }
        }

        if y_diff < 0 {
            pos.y = pos.y.saturating_sub(y_diff.abs() as usize);
        } else {
            pos.y += y_diff.abs() as usize;
        }

        pos.y = min(pos.y, self.num_lines() - 1);
        pos.x = min(pos.x, self.lines[pos.y].len());
    }

    pub fn do_scroll_up(&mut self, pos: &mut Point, height: usize) {
        let relative_y = pos.y - self.top_left.y;
        self.top_left.y = self.top_left.y.saturating_sub(height);
        pos.y = self.top_left.y + relative_y;
    }

    pub fn do_scroll_down(&mut self, pos: &mut Point, height: usize) {
        if self.num_lines() >= self.top_left.y + height {
            let relative_y = pos.y - self.top_left.y;
            self.top_left.y = min(self.num_lines() - 1, self.top_left.y + height);
            pos.y = min(self.num_lines() - 1, self.top_left.y + relative_y);
        }
    }

    pub fn do_move_to_begin(&mut self, pos: &mut Point) {
        let old = pos.x;
        pos.x = 0;
        while pos.x < self.lines[pos.y].len() {
            if !self.lines[pos.y].at(pos.x).is_whitespace() {
                break;
            }

            pos.x += 1;
        }

        if pos.x == old {
            pos.x = 0;
        }
    }

    pub fn do_move_to_end(&mut self, pos: &mut Point) {
        pos.x = self.lines[pos.y].len();
    }

    pub fn add_cursor(&mut self, position: Point) {
        self.cursors.push(Cursor::new(position));
    }

    pub fn clear_cursors(&mut self) {
        self.cursors.truncate(1);
        self.cursors[0].clear_selection();
    }

    pub fn adjust_top_left(&mut self, height: usize, width: usize) {
        let pos = &mut self.cursors[0].selection.start;
        // Scroll Up.
        if pos.y < self.top_left.y {
            self.top_left.y = pos.y;
        }

        // Scroll Down.
        if pos.y >= self.top_left.y + height {
            self.top_left.y = pos.y - height + 1;
        }

        // Scroll Right.
        if pos.x >= self.top_left.x + width {
            self.top_left.x = pos.x - width + 1;
        }

        // Scroll Left.
        if pos.x < self.top_left.x {
            self.top_left.x = pos.x;
        }

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
        /*
        match action {
            Action::Insert { pos, num, .. } => {
                pos.y = pos.y;
                pos.x = pos.x;
                for _ in 0..*num {
                    self.delete();
                }
            }
            Action::Remove { pos, text, .. } => {
                pos.y = pos.y;
                pos.x = pos.x;
                for ch in text.chars() {
                    self.insert(ch);
                }
            }
        }
        */
    }

    /// Applies `action`.
    pub fn redo_action(&mut self, action: &Action) {
        /*
        match action {
            Action::Insert { pos, text, .. } => {
                pos.y = pos.y;
                pos.x = pos.x;
                for ch in text.chars() {
                    self.insert(ch);
                }
            }
            Action::Remove { pos, num, .. } => {
                pos.y = pos.y;
                pos.x = pos.x;
                for _ in 0..*num {
                    self.delete();
                }
            }
        }
        */
    }

    pub fn commit_actions(&mut self) {
        /*
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
        */
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
