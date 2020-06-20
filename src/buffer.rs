use crate::editorconfig::{EditorConfig, EndOfLine, IndentStyle};
use std::cmp::min;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use crate::diff::*;
use crate::highlight::{Highlight, Style};
use crate::language::{Language, LANGS};

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
    lang: &'static Language,
    cursors: Vec<Cursor>,
    top_left: Point,
    file: Option<PathBuf>,
    backup_file: Option<PathBuf>,
    original_hash: u64,
    modified: bool,
    lines: Vec<Line>,
    config: EditorConfig,
    selection: Option<Selection>,
    undo_stack: Vec<Diff>,
    redo_stack: Vec<Diff>,
    version: usize,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            display_name: "".to_owned(),
            lang: &crate::language::PLAIN,
            cursors: vec![Cursor::new(Point::new(0, 0))],
            top_left: Point { x: 0, y: 0 },
            file: None,
            backup_file: None,
            original_hash: 0,
            modified: false,
            lines: vec![Line::new()],
            config: EditorConfig::default(),
            selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            version: 1,
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

        // Look for and set the language definition.
        let mut matched = None;
        'outer: for lang in LANGS {
            for filename in lang.filenames {
                if path.ends_with(filename) {
                    matched = Some(lang);
                    break 'outer;
                }
            }

            for ext in lang.extensions {
                match path.extension() {
                    Some(ext2) if ext2 == std::ffi::OsStr::new(ext) => {
                        matched = Some(lang);
                        break 'outer;
                    }
                    _ => {}
                }
            }
        }

        if let Some(lang) = matched {
            buffer.lang = lang;
        }

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

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn language(&self) -> &'static Language {
        self.lang
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.file.as_ref()
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
        for line in &self.lines {
            s += line.as_str();
            s.push('\n');
        }
        s
    }

    pub fn highlight(&self, lineno: usize, from: usize) -> Vec<(Style, &str)> {
        assert!(lineno < self.num_lines());
        // TODO: Cache highlight states to improve the highlighting performance.
        let mut h = Highlight::new(self.lang);
        for (y, line) in self.lines.iter().enumerate() {
            let mut spans = h.highlight_line(line.as_str());
            if y == lineno {
                let mut i = 0;
                loop {
                    let (_, span) = match spans.first_mut() {
                        Some(span) => span,
                        None => break,
                    };

                    if i + span.len() >= from {
                        *span = &span[from - i..];
                        break;
                    }

                    i += span.len();
                    spans.remove(0);
                }
                return spans;
            }
        }

        unreachable!();
    }

    pub fn line_at(&self, lineno: usize) -> &Line {
        &self.lines[lineno]
    }

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    fn process_command(&mut self, cmd: Command) {
        let mut end_selection = true;
        let mut removed_cursors = HashSet::new();
        for cursor_i in 0..self.cursors.len() {
            if removed_cursors.contains(&cursor_i) {
                continue;
            }

            let mut cursor = self.cursors[cursor_i].clone();
            let prev_num_lines = self.num_lines();
            match cmd {
                Command::Insert(ch) => {
                    self.remove_selection(&mut cursor);
                    self.do_insert(cursor.position_mut(), ch);
                }
                Command::Tab(after_newline) => {
                    self.remove_selection(&mut cursor);
                    self.do_tab(cursor.start_mut(), after_newline);
                }
                Command::Backspace => {
                    if cursor.is_selection() {
                        self.remove_selection(&mut cursor);
                    } else {
                        self.do_backspace(cursor.start_mut());
                    }
                },
                Command::Delete => {
                    if cursor.is_selection() {
                        self.remove_selection(&mut cursor);
                    } else {
                        self.do_delete(cursor.start_mut());
                    }
                }
                Command::Truncate => {
                    if cursor.is_selection() {
                        self.remove_selection(&mut cursor);
                    } else {
                        self.do_truncate(cursor.start_mut());
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

            if end_selection {
                cursor.clear_selection();
            }
            self.cursors[cursor_i] = cursor.clone();

            // Move other cursors after the current one if a newline is inserted
            // or removed.
            let num_lines = self.num_lines();
            for (i, c) in self.cursors.iter_mut().enumerate() {
                if i != cursor_i && c.start().y >= cursor.start().y {
                    let diff = num_lines as isize - prev_num_lines as isize;
                    c.start_mut().move_y_by(diff);
                    c.end_mut().move_y_by(diff);
                }
            }

            // Look for duplicated or out-of-range cursors.
            let num_cursors = self.cursors.len();
            for i in 0..num_cursors {
                let c = self.cursors[i].clone();
                let duplicated =
                self.cursors.iter().enumerate().any(|(j, other)|
                i != j && !removed_cursors.contains(&j) && c.intersects_with(other));
                let out_of_range =
                    c.end().y >= num_lines
                    || c.end().x > self.lines[c.end().y].len();
                if duplicated || out_of_range {
                    removed_cursors.insert(i);
                }
            }
        }

        // Remove cursors.
        let mut i = 0;
        self.cursors.retain(|_| (!removed_cursors.contains(&i), i += 1).0);
        debug_assert!(self.cursors.len() > 0);
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

    pub fn remove_selection(&mut self, cursor: &mut Cursor) -> String {
        if !cursor.is_selection() {
            return String::new();
        }

        let removed = self.copy_selection(cursor);
        warn!("not removed = '{}'", removed);
        self.apply_diff(
            Diff::Remove(*cursor.start(), *cursor.end(), removed.clone()));
        self.modified = true;
        cursor.clear_selection();
        removed
    }

    pub fn copy_selection(&self, cursor: &Cursor) -> String {
        // A example text:
        //
        //    start
        //    V
        // ...123
        // abcdef
        // ghi...
        //    ^
        //    end
        let mut pos = cursor.start().clone();
        let mut copied = String::new();
        // Copy "123" and "abcdef".
        while pos.y < cursor.end().y {
            copied += self.lines[pos.y].substr_from(pos.x);
            pos.x = 0;
            pos.y += 1;
            copied.push('\n');
        }

        // Copy "ghi"
        copied += self.lines[pos.y].substr(pos.x, cursor.end().x);
        copied
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

    pub fn apply_diff(&mut self, diff: Diff) -> Point {
        match diff {
            Diff::Move(_) => { /* Move does not modify the buffer. */ }
            _ => { self.version += 1; }
        }

        let new_pos = diff.apply(&mut self.lines);
        self.undo_stack.push(Diff::Move(self.cursors.clone()));
        self.undo_stack.push(diff);
        self.redo_stack.clear();
        new_pos
    }

    fn do_insert(&mut self, pos: &mut Point, ch: char) {
        *pos = self.apply_diff(Diff::InsertChar(*pos, ch));
        self.modified = true;

        // Auto indentation.
        if ch == '\n' {
            self.do_tab(pos, true);
        }
    }

    pub fn do_backspace(&mut self, pos: &mut Point) {
        if pos.y == 0 && pos.x == 0 {
            return;
        }

        let indent_len = self.lines[pos.y].indent().len();
        if 0 < pos.x && pos.x <= indent_len {
            // Decrease the indentation level.
            let mut num_remove = pos.x % self.config.indent_size;
            if num_remove == 0 {
                num_remove = self.config.indent_size;
            }

            for _ in 0..num_remove {
                let ch = self.lines[pos.y].at(pos.x - 1);
                *pos = self.apply_diff(Diff::BackspaceChar(*pos, ch));
            }
        } else {
            let removed_char =
            if pos.x == 0 { '\n' } else { self.lines[pos.y].at(pos.x - 1) };
            *pos = self.apply_diff(Diff::BackspaceChar(*pos, removed_char));
        }

        self.modified = true;
    }

    pub fn do_delete(&mut self, pos: &mut Point) {
        let eol = pos.x == self.lines[pos.y].len();
        if pos.y == self.num_lines() - 1 && eol {
            return;
        }

        let (removed_char, end) =
            if eol {
                ('\n', Point::new(0, pos.y + 1))
            } else {
                (self.lines[pos.y].at(pos.x), Point::new(pos.x + 1, pos.y))
            };
        self.apply_diff(Diff::Remove(*pos, end, removed_char.to_string()));
        self.modified = true;
    }

    pub fn do_truncate(&mut self, pos: &mut Point) {
        if pos.y == self.num_lines() - 1 && pos.x == self.lines[pos.y].len() {
            return;
        }

        self.modified = true;
        if pos.x == self.lines[pos.y].len() {
            let end = Point::new(0, pos.y + 1);
            self.apply_diff(Diff::Remove(*pos, end, '\n'.to_string()));
        } else {
            let mut end = *pos;
            end.x = self.lines[pos.y].len();
            let removed = self.lines[pos.y].substr_from(pos.x).to_owned();
            self.apply_diff(Diff::Remove(*pos, end, removed));
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
        if pos.y < height {
            pos.y = 0;
        } else {
            let relative_y = pos.y - self.top_left.y;
            self.top_left.y = self.top_left.y.saturating_sub(height);
            pos.y = self.top_left.y + relative_y;
        }

        pos.x = min(pos.x, self.lines[pos.y].len());
    }

    pub fn do_scroll_down(&mut self, pos: &mut Point, height: usize) {
        if self.num_lines() < self.top_left.y + height {
            pos.y = self.num_lines() - 1;
        } else {
            let relative_y = pos.y - self.top_left.y;
            self.top_left.y = min(self.num_lines() - 1, self.top_left.y + height);
            pos.y = min(self.num_lines() - 1, self.top_left.y + relative_y);
        }

        pos.x = min(pos.x, self.lines[pos.y].len());
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
        let pos = &mut self.cursors[0].selection_mut().end;
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

    pub fn cut(&mut self) -> String {
        let mut clipboard = String::new();
        let cursors = self.cursors.clone();
        let mut new_cursors = Vec::with_capacity(self.cursors.len());
        for mut cursor in cursors {
            clipboard.push_str(&self.remove_selection(&mut cursor));
            cursor.clear_selection();
            new_cursors.push(cursor);
        }

        if self.cursors.len() == 1 {
            self.selection = None;
        }

        self.cursors = new_cursors;
        clipboard
    }

    pub fn copy(&mut self) -> String {
        let mut clipboard = String::new();
        for cursor in &self.cursors {
            clipboard.push_str(&self.copy_selection(cursor));
        }
        for cursor in &mut self.cursors {
            cursor.clear_selection();
        }
        clipboard
    }

    pub fn paste(&mut self, clipboard: &str) {
        let mut cursors = self.cursors.clone();
        let lines: Vec<&str> = clipboard.lines().collect();
        if cursors.len() == lines.len() {
            for (i, line) in lines.iter().enumerate() {
                self.remove_selection(&mut cursors[i]);
                for ch in line.chars() {
                    self.do_insert(cursors[i].position_mut(), ch);
                    cursors[i].clear_selection();
                }
            }
        } else {
            // Use lines as a single string.
            for cursor in &mut cursors {
                self.remove_selection(cursor);
                let mut iter = lines.iter().enumerate().peekable();
                while let Some((i, line)) = iter.next() {
                    if i > 0 {
                        self.do_insert(cursor.position_mut(), '\n');
                        cursor.clear_selection();
                    }

                    for ch in line.chars() {
                        self.do_insert(cursor.position_mut(), ch);
                        cursor.clear_selection();
                    }
                }
            }
        }

        self.cursors = cursors;
    }

    pub fn add_undo_stop(&mut self) {
        match self.undo_stack.last() {
            Some(Diff::Stop) => {}
            _ => { self.undo_stack.push(Diff::Stop); }
        }
    }

    pub fn undo(&mut self) {
        match self.undo_stack.last() {
            Some(Diff::Stop) => { self.undo_stack.pop(); }
            _ => {}
        }

        trace!("undo: {:?}", self.undo_stack.last());
        while let Some(diff) = self.undo_stack.pop() {
            match &diff {
                Diff::Stop => break,
                Diff::Move(cursors) => self.cursors = cursors.to_owned(),
                _ => diff.revert(&mut self.lines),
            }
            self.redo_stack.push(diff);
        }
    }

    pub fn redo(&mut self) {
        match self.redo_stack.last() {
            Some(Diff::Stop) => { self.redo_stack.pop(); }
            _ => {}
        }

        trace!("redo: {:?}", self.redo_stack.last());
        while let Some(diff) = self.redo_stack.pop() {
            match &diff {
                Diff::Stop => break,
                Diff::Move(cursors) => self.cursors = cursors.to_owned(),
                _ => { diff.apply(&mut self.lines); }
            }
            self.undo_stack.push(diff);
        }
    }
}
