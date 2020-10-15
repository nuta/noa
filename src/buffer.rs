use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::path::{Path, PathBuf};
use std::ops::RangeInclusive;
use std::cmp::{max, min};
use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use crate::highlight::{Highlighter, Span};
use crate::language::Language;
use crate::editorconfig::{EditorConfig, IndentStyle};
use crate::rope::*;

pub fn compute_str_checksum(string: &str) -> u32 {
    fxhash::hash32(string)
}

fn remove_range(
    buf: &mut Rope,
    range: &Range,
    next_cursor: Option<&Cursor>,
    new_cursors: &mut Vec<Cursor>
) {
    // Remove the text in the range.
    buf.remove(&range);

    // Move cursors after the current cursor.
    let front = range.front();
    let end = range.back();
    let num_newlines_deleted = end.y - front.y;
    for c2 in new_cursors.iter_mut() {
        match c2 {
            Cursor::Normal { pos, .. } if pos.y == end.y => {
                pos.x = front.x + (pos.x - end.x);
                pos.y = front.y;
            }
            Cursor::Normal { pos, .. } => {
                pos.y -= num_newlines_deleted;
            }
            Cursor::Selection(_) => {
                unreachable!();
            }
        }
    }

    // Preserve the current cursor if it's unique (no other cursors at
    // the same position).
    match next_cursor {
        Some(Cursor::Normal { pos, .. }) if pos == front => {}
        _ => {
            new_cursors.push(Cursor::new(front.y, front.x));
        }
    }
}

fn backup_path(backup_dir: &Path, base: &str, revision: usize) -> PathBuf {
    backup_dir.join(format!("{}.{}", base, revision))
}

static NEXT_BUFFER_ID: AtomicUsize = AtomicUsize::new(0);
static NEXT_SNAPSHOT_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct Snapshot {
    pub id: usize,
    pub buffer_id: BufferId,
    pub buf: Rope,
    pub main_cursor: Option<Point>,
    pub modified_line: usize,
}

impl Snapshot {
    pub fn new(
        buffer_id: BufferId,
        buf: Rope,
        main_cursor: Option<Point>,
        modified_line: usize,
    ) -> Snapshot {
        Snapshot {
            id: NEXT_SNAPSHOT_ID.fetch_add(1, Ordering::SeqCst),
            buffer_id,
            buf,
            main_cursor,
            modified_line,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct BufferId(usize);

impl BufferId {
    pub fn alloc() -> BufferId {
        BufferId(NEXT_BUFFER_ID.fetch_add(1, Ordering::SeqCst))
    }
}

pub struct Buffer {
    id: BufferId,
    buf: Rope,
    name: String,
    version: usize,
    file: Option<PathBuf>,
    tmpfile: NamedTempFile,
    tmpfile_version: Rope,
    cursors: Vec<Cursor>,
    undo_stack: Vec<Rope>,
    redo_stack: Vec<Rope>,
    #[allow(unused)]
    lang: &'static Language,
    highlighter: Highlighter,
    config: EditorConfig,
}

impl Buffer {
    pub fn new() -> Buffer {
        let lang = &crate::language::PLAIN;
        let mut buffer = Buffer {
            id: BufferId::alloc(),
            buf: Rope::new(),
            version: 1,
            name: String::new(),
            file: None,
            tmpfile: NamedTempFile::new().unwrap(),
            tmpfile_version: Rope::new(),
            cursors: vec![Cursor::new(0, 0)],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            lang,
            highlighter: Highlighter::new(lang),
            config: EditorConfig::default(),
        };

        buffer.mark_undo_point();
        buffer
    }

    #[cfg(test)]
    pub fn from_str(text: &str) -> Buffer {
        let mut buf = Buffer::new();
        buf.insert(text);
        buf
    }

    pub fn open_file(path: &Path) -> std::io::Result<Buffer> {
        let lang = &crate::language::PLAIN;
        let file = std::fs::File::open(path)?;
        let mut buffer = Buffer {
            id: BufferId::alloc(),
            buf: Rope::from_reader(file)?,
            version: 1,
            name: String::new(),
            file: Some(path.canonicalize()?),
            tmpfile: NamedTempFile::new()?,
            tmpfile_version: Rope::new(),
            cursors: vec![Cursor::new(0, 0)],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            lang,
            highlighter: Highlighter::new(lang),
            config: EditorConfig::resolve(path),
        };

        buffer.mark_undo_point();
        Ok(buffer)
    }

    pub fn set_text(&mut self, text: &str) {
        self.buf.clear();
        self.buf.insert(&Point::new(0, 0), text);

        let mut pos = match self.cursors[0] {
            Cursor::Normal { pos } => pos,
            Cursor::Selection(Range { end, .. }) => end,
        };

        pos.y = min(pos.y, self.buf.num_lines().saturating_sub(1));
        pos.x = min(pos.x, self.buf.line_len(pos.y));
        self.cursors = vec![Cursor::Normal { pos }];
    }

    pub fn id(&self) -> BufferId {
        self.id
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn num_lines(&self) -> usize {
        self.buf.num_lines()
    }

    pub fn line_len(&self, y: usize) -> usize {
        self.buf.line_len(y)
    }

    pub fn is_dirty(&self) -> bool {
        self.undo_stack.len() != 1
    }

    pub fn checksum(&self) -> u32 {
        compute_str_checksum(&self.text())
    }

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn lang(&self) -> &'static Language {
        &self.lang
    }

    pub fn file(&self) -> &Option<PathBuf> {
        &self.file
    }

    pub fn config(&self) -> &EditorConfig {
        &self.config
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name<T: Into<String>>(&mut self, name: T) {
        self.name = name.into();
    }

    pub fn update_tmpfile(&mut self) {
        if self.tmpfile_version != self.buf {
            std::fs::write(self.tmpfile.path(), self.text()).ok();
        }

        self.tmpfile_version = self.buf.clone();
    }

    pub fn tmpfile(&self) -> &Path {
        self.tmpfile.path()
    }

    pub fn path(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    pub fn text(&self) -> String {
        self.buf.text()
    }

    pub fn line(&self, line: usize) -> ropey::RopeSlice {
        self.buf.line(line)
    }

    pub fn modified_line(&self) -> &Option<usize> {
        &self.buf.modified_line()
    }

    pub fn snapshot(&self) -> Snapshot {
        let main_cursor = match self.cursors[0] {
            Cursor::Normal { pos, .. } => Some(pos),
            _ => None,
        };

        let modified_line = self.modified_line().unwrap_or(0);
        Snapshot::new(self.id, self.buf.clone(), main_cursor, modified_line)
    }

    pub fn save_without_backup(&self) -> std::io::Result<()> {
        if let Some(path) = &self.file {
            self.buf.save_into_file(path)
        } else {
            Ok(())
        }
    }

    pub fn save(&self, backup_dir: &Path) -> std::io::Result<()> {
        if let Some(path) = &self.file {
            let base = path.to_str().unwrap().replace('/', ".");
            fs::create_dir_all(backup_dir)?;
            fs::rename(
                backup_path(backup_dir, &base, 2),
                backup_path(backup_dir, &base, 3),
            ).ok();
            fs::rename(
                backup_path(backup_dir, &base, 1),
                backup_path(backup_dir, &base, 2),
            ).ok();
            fs::copy(
                path,
                backup_path(backup_dir, &base, 1)
            ).ok();
            self.buf.save_into_file(path)
        } else {
            Ok(())
        }
    }

    pub fn cursors(&self) -> &[Cursor] {
        &self.cursors
    }

    pub fn main_cursor_pos(&self) -> &Point {
        match &self.cursors[0] {
            Cursor::Normal { pos } => pos,
            Cursor::Selection(range) => &range.end,
        }
    }

    pub fn set_cursors(&mut self, cursors: Vec<Cursor>) {
        self.cursors = cursors;
        self.sort_and_merge_cursors();
    }

    pub fn move_cursors(
        &mut self,
        up: usize,
        down: usize,
        left: usize,
        right: usize
    ) {
        for cursor in &mut self.cursors {
            // Cancel the selection.
            match cursor {
                Cursor::Normal { .. } => {}
                Cursor::Selection(range) => {
                    let pos = if left > 0 || up > 0 {
                        range.front()
                    } else {
                        range.back()
                    };

                    *cursor = Cursor::new(pos.y, pos.x);
                }
            };

            // Move the cursor.
            match cursor {
                Cursor::Normal { pos, .. } => {
                    pos.move_by(&self.buf, up, down, left, right);
                }
                Cursor::Selection(_) => unreachable!()
            };
        }

        self.sort_and_merge_cursors();
    }

    pub fn move_to_end_of_line(&mut self) {
        for cursor in &mut self.cursors {
            let y = match cursor {
                Cursor::Normal { pos, .. } => pos.y,
                Cursor::Selection(Range { end, .. }) => end.y,
            };

            *cursor = Cursor::new(y, self.buf.line_len(y));
        }

        self.sort_and_merge_cursors();
    }

    pub fn move_to_beginning_of_line(&mut self) {
        for cursor in &mut self.cursors {
            let y = match cursor {
                Cursor::Normal { pos, .. } => pos.y,
                Cursor::Selection(Range { end, .. }) => end.y,
            };

            *cursor = Cursor::new(y, 0);
        }

        self.sort_and_merge_cursors();
    }

    pub fn move_to_prev_word(&mut self) {
        for cursor in &mut self.cursors {
            let pos = match cursor {
                Cursor::Normal { pos, .. } => pos,
                Cursor::Selection(Range { start, .. }) => start,
            };

            let new_pos = self.buf.prev_word_end(&pos);
            *cursor = Cursor::new(new_pos.y, new_pos.x);
        }

        self.sort_and_merge_cursors();
    }

    pub fn move_to_next_word(&mut self) {
        for cursor in &mut self.cursors {
            let pos = match cursor {
                Cursor::Normal { pos, .. } => pos,
                Cursor::Selection(Range { start, .. }) => start,
            };

            let new_pos = self.buf.next_word_end(&pos);
            *cursor = Cursor::new(new_pos.y, new_pos.x);
        }

        self.sort_and_merge_cursors();
    }

    pub fn select(
        &mut self,
        up: usize,
        down: usize,
        left: usize,
        right: usize
    ) {
        for cursor in &mut self.cursors {
            let (start, mut end) = match cursor {
                Cursor::Normal { pos, .. } => (*pos, *pos),
                Cursor::Selection(Range { start, end }) => (*start, *end),
            };

            end.move_by(&self.buf, up, down, left, right);
            *cursor = Cursor::Selection(Range::from_points(start, end));
        }

        self.sort_and_merge_cursors();
    }

    pub fn select_until_end_of_line(&mut self) {
        for cursor in &mut self.cursors {
            let (start, mut end) = match cursor {
                Cursor::Normal { pos, .. } => (*pos, *pos),
                Cursor::Selection(Range { start, end }) => (*start, *end),
            };

            end.x = self.buf.line_len(end.y);
            *cursor = Cursor::Selection(Range::from_points(start, end));
        }

        self.sort_and_merge_cursors();
    }

    pub fn insert_char(&mut self, ch: char) {
        self.insert(&ch.to_string())
    }

    pub fn insert(&mut self, string: &str) {
        self.buf.reset_modified_line();

        let mut new_cursors = Vec::new();
        for c in self.cursors.iter().rev() {
            let (remove, insert_at, end) = match c {
                Cursor::Normal { pos, .. } => {
                    (None, pos, pos)
                }
                Cursor::Selection(range) => {
                    (Some(range), range.front(), range.back())
                }
            };

            if let Some(remove) = remove {
                self.buf.remove(&remove);
            }

            // Handle insertion at the end of file.
            if insert_at.y == self.num_lines() && string != "\n" {
                debug_assert!(insert_at.x == 0);
                self.buf.insert(insert_at, "\n");
            }

            self.buf.insert(insert_at, string);

            let num_newlines_added = string.matches('\n').count();
            let num_newlines_deleted =
                remove.map(|r| r.back().y - r.front().y).unwrap_or(0);
            let y_diff = num_newlines_added.saturating_sub(num_newlines_deleted);

            // Move cursors after the current cursor.
            for c2 in new_cursors.iter_mut() {
                match c2 {
                    Cursor::Normal { pos, .. } if pos.y == end.y => {
                        pos.x = insert_at.x + (pos.x - end.x);
                        pos.y = insert_at.y;
                    }
                    Cursor::Normal { pos, .. } => {
                        pos.y += y_diff;
                    }
                    Cursor::Selection(_) => {
                        unreachable!();
                    }
                }
            }

            let x_diff = string.rfind('\n')
                .map(|x| string.len() - x - 1)
                .unwrap_or_else(|| string.len());

            let y = insert_at.y + y_diff;
            let x = if string.contains('\n') {
                x_diff
            } else {
                insert_at.x + x_diff
            };

            let new_pos = Point::new(y, x);
            new_cursors.push(Cursor::new(new_pos.y, new_pos.x));
        }

        self.set_cursors(new_cursors);
    }

    pub fn clear(&mut self) {
        self.buf = Rope::new();
        self.cursors = vec![Cursor::new(0, 0)];
    }

    fn indent_size(&self, y: usize) -> usize {
        let mut n = 0;
        let line = self.buf.line(y);
        'outer: for c in line.chunks() {
            for ch in c.chars() {
                trace!("ch='{}' {}, n={}", ch, ch.is_ascii_whitespace(), n);
                if !ch.is_ascii_whitespace() {
                    break 'outer;
                }

                n += 1;
            }
        }

        n
    }

    pub fn tab(&mut self) {
        self.buf.reset_modified_line();
        let mut new_cursors = Vec::new();
        for c in self.cursors.iter().rev() {
            let pos = match c {
                Cursor::Normal { pos, .. } => {
                    pos
                }
                Cursor::Selection(range) => {
                    range.front()
                }
            };

            // Should we do auto-indent?
            let auto_indent = pos.x <= self.indent_size(pos.y);
            let x;
            if auto_indent {
                let prev_indent_size = if pos.y > 0 {
                    self.indent_size(pos.y - 1)
                } else {
                    0
                };
                let indent_size = max(prev_indent_size, self.config.indent_size);
                let num_chars = indent_size - (pos.x % indent_size);

                x = pos.x + num_chars;
                let ch = match self.config.indent_style {
                    IndentStyle::Space => ' ',
                    IndentStyle::Tab => '\t',
                };
                for _ in 0..num_chars {
                    self.buf.insert_char(pos, ch);
                }
            } else {
                // Not auto indent; the user just wants to input '\t'.
                self.buf.insert_char(pos, '\t');
                x = pos.x + 1;
            }

            new_cursors.push(Cursor::new(pos.y, x));
        }

        self.set_cursors(new_cursors);
    }

    // Decrease indent levels.
    pub fn back_tab(&mut self) {
        self.buf.reset_modified_line();
        let mut new_cursors = Vec::new();
        let mut ys = HashSet::new();
        for c in self.cursors.iter().rev() {
            let pos = match c {
                Cursor::Normal { pos, .. } => {
                    pos
                }
                Cursor::Selection(range) => {
                    range.front()
                }
            };

            let n = min(
                self.indent_size(pos.y),
                if pos.x % self.config.indent_size == 0 {
                    self.config.indent_size
                } else {
                    pos.x % self.config.indent_size
                }
            );
            if n > 0 && !ys.contains(&pos.y) {
                let start = Point::new(pos.y, 0);
                let end = Point::new(pos.y, n);
                self.buf.remove(&Range::from_points(start, end));
                new_cursors.push(Cursor::new(pos.y, pos.x.saturating_sub(n)));
                ys.insert(pos.y);
            } else {
                new_cursors.push(Cursor::new(pos.y, pos.x));
            }
        }

        self.set_cursors(new_cursors);
    }

    pub fn backspace(&mut self) {
        self.buf.reset_modified_line();

        let mut new_cursors = Vec::new();
        let mut iter = self.cursors.iter().rev().peekable();
        while let Some(c) = iter.next() {
            // Determine the range to be deleted.
            let range = match c {
                Cursor::Normal { pos, .. } => {
                    let start = if pos.y == 0 && pos.x == 0 {
                        new_cursors.push(c.clone());
                        continue;
                    } else if pos.x == 0 {
                        Point::new(pos.y - 1, self.buf.line_len(pos.y - 1))
                    } else {
                        Point::new(pos.y, pos.x - 1)
                    };

                    Range::from_points(start, *pos)
                }
                Cursor::Selection(range) => {
                    range.clone()
                }
            };

            remove_range(&mut self.buf, &range, iter.peek().copied(), &mut new_cursors);
        }

        self.set_cursors(new_cursors);
    }

    pub fn delete(&mut self) {
        self.buf.reset_modified_line();

        let mut new_cursors = Vec::new();
        let mut iter = self.cursors.iter().rev().peekable();
        while let Some(c) = iter.next() {
            // Determine the range to be deleted.
            let range = match c {
                Cursor::Normal { pos, .. } => {
                    let max_y = self.buf.num_lines();
                    let max_x = self.buf.line_len(pos.y);
                    let end = if pos.y == max_y && pos.x == max_x {
                        new_cursors.push(c.clone());
                        continue;
                    } else if pos.x == max_x {
                        Point::new(pos.y + 1, 0)
                    } else {
                        Point::new(pos.y, pos.x + 1)
                    };

                    Range::from_points(*pos, end)
                }
                Cursor::Selection(range) => {
                    range.clone()
                }
            };

            remove_range(&mut self.buf, &range, iter.peek().copied(), &mut new_cursors);
        }

        self.set_cursors(new_cursors);
    }

    pub fn truncate(&mut self) {
        self.buf.reset_modified_line();

        self.select_until_end_of_line();
        self.delete();
    }

    pub fn mark_undo_point(&mut self) {
        self.undo_stack.push(self.buf.clone());
        self.version += 1;
    }

    pub fn undo(&mut self) {
        if self.undo_stack.len() == 1 && self.buf.is_empty() {
            return;
        }

        if let Some(top) = self.undo_stack.last() {
            if *top == self.buf {
                self.undo_stack.pop();
            }
        }

        if let Some(buf) = self.undo_stack.pop() {
            self.redo_stack.push(self.buf.clone());
            self.buf = buf;
        }
    }

    pub fn redo(&mut self) {
        if let Some(buf) = self.redo_stack.pop() {
            self.undo_stack.push(self.buf.clone());
            self.buf = buf;
        }
    }

    /// Sorts the cursors and removes overlapped ones. Don't forget to call this
    /// method when you made a change.
    fn sort_and_merge_cursors(&mut self) {
        debug_assert!(!self.cursors.is_empty());

        self.cursors.sort();
        let duplicated =
            self.cursors
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    match c {
                        Cursor::Normal { pos, .. } => {
                            (&self.cursors[..i])
                                .iter()
                                .any(|other| {
                                    match other {
                                        Cursor::Normal { pos: ref other } => {
                                            *pos == *other
                                        }
                                        _ => unreachable!()
                                    }
                                })
                        }
                        Cursor::Selection(range) => {
                            (&self.cursors[..i])
                                .iter()
                                .any(|other| {
                                    match other {
                                        Cursor::Selection(ref other) => {
                                            range.overlaps_with(other)
                                        }
                                        _ => unreachable!()
                                    }
                                })
                        }
                    }
                });

        let mut new_cursors = Vec::new();
        for (cursor, skip) in self.cursors.iter().zip(duplicated) {
            if !skip {
                new_cursors.push(cursor.clone());
            }
        }

        self.cursors = new_cursors;
    }

    pub fn current_word(&self) -> Option<String> {
        let pos = match &self.cursors[0] {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection(Range { start, .. }) => start,
        };

        self.buf.word_at(pos).map(|(_, word)| word)
    }

    pub fn current_word_range(&self) -> Option<Range> {
        let pos = match &self.cursors[0] {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection(Range { start, .. }) => start,
        };

        self.buf.word_at(pos).map(|(range, _)| range)
    }

    pub fn find(&mut self, needle: &str) -> Vec<Range> {
        if needle.is_empty() {
            return Vec::new();
        }

        // TODO: Implement a well-known better algorithm.
        let needle_chars: Vec<char> = needle.chars().collect();
        let mut matches = Vec::new();
        let mut y = 0;
        let mut x = 0;
        for ch in self.buf.chars() {
            for m in &mut matches {
                match m {
                    (0, _) => {
                        // this `m` does not match.
                    }
                    (next_index, _) if *next_index == needle_chars.len() => {
                        // This `m` matches to the needle.
                    }
                    (next_index, _) if needle_chars[*next_index] == ch => {
                        // This `m` partially matches to the needle. Go to the
                        // next char...
                        *next_index += 1;
                    }
                    (next_index, _) => {
                        // this `m` does not match.
                        *next_index = 0;
                    }
                }
            }

            if ch == needle_chars[0] {
                matches.push((1, Point::new(y, x)));
            }

            if ch == '\n' {
                y += 1;
                x = 0;
            } else {
                x += 1;
            }
        }

        let y_len = needle.matches('\n').count();
        let last_newline_idx = needle.rfind('\n');

        matches.iter()
            .filter(|(index, _)| *index == needle_chars.len())
            .map(|(_, start)| {
                let x = last_newline_idx
                    .map(|i| needle.len() - i - 1)
                    .unwrap_or_else(|| start.x + needle.len());
                Range::new(start.y, start.x, start.y + y_len, x)
            })
            .collect::<Vec<Range>>()
    }

    pub fn select_by_ranges(&mut self, selections: &[Range]) {
        self.cursors.clear();
        for selection in selections {
            self.cursors.push(Cursor::Selection(selection.clone()));
        }

        self.sort_and_merge_cursors();
    }

    pub fn highlighted_line(&self, y: usize) -> &[Span] {
        self.highlighter.line(y)
    }

    pub fn highlight(&mut self, lines: RangeInclusive<usize>) {
        self.highlighter.highlight(self.snapshot(), lines, &self.cursors);
    }
}

impl PartialEq for Buffer {
    fn eq(&self, other: &Buffer) -> bool {
        self.id == other.id
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn insertion_and_deletion() {
        let mut b = Buffer::new();
        b.backspace();
        b.insert("Hello");
        b.insert(" World?");
        assert_eq!(b.text(), "Hello World?");
        b.backspace();
        assert_eq!(b.text(), "Hello World");
        b.insert_char('!');
        assert_eq!(b.text(), "Hello World!");
        b.move_cursors(0, 0, 1, 0); // Move left
        b.delete();
        assert_eq!(b.text(), "Hello World");
        b.delete();
        assert_eq!(b.text(), "Hello World");
    }

    #[test]
    fn insert_at_eof() {
        let mut b = Buffer::new();
        b.insert("abc");
        b.move_cursors(0, 1, 0, 0); // Move down
        assert_eq!(b.cursors(), &[Cursor::new(1, 0)]);
        b.insert_char('x');
        assert_eq!(b.text(), "abc\nx");
        assert_eq!(b.cursors(), &[Cursor::new(1, 1)]);
        b.insert_char('y');
        assert_eq!(b.text(), "abc\nxy");
        assert_eq!(b.cursors(), &[Cursor::new(1, 2)]);
    }

    #[test]
    fn single_cursor() {
        let mut b = Buffer::new();
        b.move_cursors(1, 0, 0, 0); // Do nothing
        b.insert("A\nDEF\n12345");
        assert_eq!(b.cursors(), &[Cursor::new(2, 5)]);
        b.move_cursors(0, 0, 1, 0); // Move right
        assert_eq!(b.cursors(), &[Cursor::new(2, 4)]);
        b.move_cursors(1, 0, 0, 0); // Move up
        assert_eq!(b.cursors(), &[Cursor::new(1, 3)]);
        b.move_cursors(0, 3, 0, 0); // Move down
        assert_eq!(b.cursors(), &[Cursor::new(3, 0)]);
        b.move_cursors(0, 0, 1, 0); // Move left
        assert_eq!(b.cursors(), &[Cursor::new(2, 5)]);
        b.move_cursors(0, 0, 0, 1); // Move right
        assert_eq!(b.cursors(), &[Cursor::new(3, 0)]);
    }

    #[test]
    fn insert_on_multi_cursors() {
        let mut b = Buffer::new();
        // abc|
        // d|e
        // |xyz
        b.insert("abc\nde\nxyz");
        b.set_cursors(vec![
            Cursor::new(0, 3),
            Cursor::new(1, 1),
            Cursor::new(2, 0),
        ]);

        // abc123|
        // d123|e
        // 123|xyz
        b.insert("123");
        assert_eq!(b.text(), "abc123\nd123e\n123xyz");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 6),
            Cursor::new(1, 4),
            Cursor::new(2, 3),
        ]);

        // abc123[
        // ]|
        // d123[
        // ]|e
        // 123[
        // ]|xyz
        b.insert("[\n]");
        assert_eq!(b.text(), "abc123[\n]\nd123[\n]e\n123[\n]xyz");
        assert_eq!(b.cursors(), &[
            Cursor::new(1, 1),
            Cursor::new(3, 1),
            Cursor::new(5, 1),
        ]);
    }

    #[test]
    fn backspace_on_multi_cursors() {
        // abc|      ab|
        // def|  =>  de|
        // xyz|      xy|
        let mut b = Buffer::new();
        b.insert("abc\ndef\nxyz");
        b.set_cursors(vec![
            Cursor::new(0, 3),
            Cursor::new(1, 3),
            Cursor::new(2, 3),
        ]);
        b.backspace();
        assert_eq!(b.text(), "ab\nde\nxy");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 2),
            Cursor::new(1, 2),
            Cursor::new(2, 2),
        ]);

        // abc|      ab|
        // 1|    =>  |
        // xy|z      x|z
        let mut b = Buffer::new();
        b.insert("abc\n1\nxyz");
        b.set_cursors(vec![
            Cursor::new(0, 3),
            Cursor::new(1, 1),
            Cursor::new(2, 2),
        ]);
        b.backspace();
        assert_eq!(b.text(), "ab\n\nxz");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 2),
            Cursor::new(1, 0),
            Cursor::new(2, 1),
        ]);

        // 1230|a|b|c|d|e|f => 123|f
        let mut b = Buffer::new();
        b.insert("1230abcdef");
        b.set_cursors(vec![
            Cursor::new(0, 4),
            Cursor::new(0, 5),
            Cursor::new(0, 6),
            Cursor::new(0, 7),
            Cursor::new(0, 8),
            Cursor::new(0, 9),
        ]);
        b.backspace();
        assert_eq!(b.text(), "123f");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 3),
        ]);

        // a|bc      |bc|12
        // |12   =>  xy|
        // xyz|
        let mut b = Buffer::new();
        b.insert("abc\n12\nxyz");
        b.set_cursors(vec![
            Cursor::new(0, 1),
            Cursor::new(1, 0),
            Cursor::new(2, 3),
        ]);
        b.backspace();
        assert_eq!(b.text(), "bc12\nxy");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 0),
            Cursor::new(0, 2),
            Cursor::new(1, 2),
        ]);

        // 0
        // |abc      0|abc|12|xyz
        // |12   =>
        // |xyz
        let mut b = Buffer::new();
        b.insert("0\nabc\n12\nxyz");
        b.set_cursors(vec![
            Cursor::new(1, 0),
            Cursor::new(2, 0),
            Cursor::new(3, 0),
        ]);
        b.backspace();
        assert_eq!(b.text(), "0abc12xyz");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
            Cursor::new(0, 4),
            Cursor::new(0, 6),
        ]);

        // ab|     =>  a|def|g
        // |c|def
        // |g
        let mut b = Buffer::new();
        b.insert("ab\ncdef\ng");
        b.set_cursors(vec![
            Cursor::new(0, 2),
            Cursor::new(1, 0),
            Cursor::new(1, 1),
            Cursor::new(2, 0),
        ]);
        b.backspace();
        assert_eq!(b.text(), "adefg");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
            Cursor::new(0, 4),
        ]);

        // ab|   =>  a|def|g
        // |c|def
        // |g
        let mut b = Buffer::new();
        b.insert("ab\ncdef\ng");
        b.set_cursors(vec![
            Cursor::new(0, 2),
            Cursor::new(1, 0),
            Cursor::new(1, 1),
            Cursor::new(2, 0),
        ]);
        b.backspace();
        assert_eq!(b.text(), "adefg");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
            Cursor::new(0, 4),
        ]);
    }

    #[test]
    fn delete_on_multi_cursors() {
        // a|Xbc|Yd
        let mut b = Buffer::new();
        b.insert("aXbcYd");
        b.set_cursors(vec![
            Cursor::new(0, 1),
            Cursor::new(0, 4),
        ]);
        b.delete();
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
            Cursor::new(0, 3),
        ]);

        // a|b|
        let mut b = Buffer::new();
        b.insert("ab");
        b.set_cursors(vec![
            Cursor::new(0, 1),
            Cursor::new(0, 2),
        ]);
        b.delete();
        assert_eq!(b.text(), "a");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
        ]);

        // a|bc
        // d|ef
        // g|hi
        let mut b = Buffer::new();
        b.insert("abc\ndef\nghi");
        b.set_cursors(vec![
            Cursor::new(0, 1),
            Cursor::new(1, 1),
            Cursor::new(2, 1),
        ]);
        b.delete();
        assert_eq!(b.text(), "ac\ndf\ngi");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
            Cursor::new(1, 1),
            Cursor::new(2, 1),
        ]);

        // a|
        // b|X
        // c|Y
        // d|
        let mut b = Buffer::new();
        b.insert("a\nbX\ncY\nd");
        b.set_cursors(vec![
            Cursor::new(0, 1),
            Cursor::new(1, 1),
            Cursor::new(2, 1),
            Cursor::new(3, 1),
        ]);
        b.delete();
        assert_eq!(b.text(), "ab\nc\nd");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
            Cursor::new(0, 2),
            Cursor::new(1, 1),
            Cursor::new(2, 1),
        ]);

        // ab|
        // cde|
        let mut b = Buffer::new();
        b.insert("ab\ncde");
        b.set_cursors(vec![
            Cursor::new(0, 2),
            Cursor::new(1, 3),
        ]);
        b.delete();
        assert_eq!(b.text(), "abcde");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 2),
            Cursor::new(0, 5),
        ]);

        // abc|
        // |d|ef
        // ghi|
        let mut b = Buffer::new();
        b.insert("abc\ndef\nghi");
        b.set_cursors(vec![
            Cursor::new(0, 3),
            Cursor::new(1, 0),
            Cursor::new(1, 1),
            Cursor::new(2, 3),
        ]);
        b.delete();
        assert_eq!(b.text(), "abcf\nghi");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 3),
            Cursor::new(1, 3),
        ]);

        // abc|     => abc|d|e|f
        // d|Xe|Yf
        let mut b = Buffer::new();
        b.insert("abc\ndXeYf");
        b.set_cursors(vec![
            Cursor::new(0, 3),
            Cursor::new(1, 1),
            Cursor::new(1, 3),
        ]);
        b.delete();
        assert_eq!(b.text(), "abcdef");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 3),
            Cursor::new(0, 4),
            Cursor::new(0, 5),
        ]);
    }

    #[test]
    fn multibyte_characters() {
        let mut b = Buffer::new();
        b.insert("Hello 世界!");
        b.set_cursors(vec![Cursor::new(0, 7)]);
        assert_eq!(b.len(), 9);

        // Hello 世|界! => Hello |界!
        b.backspace();
        assert_eq!(b.text(), "Hello 界!");
        // Hello 世|界! => Hell|界!
        b.backspace();
        b.backspace();
        assert_eq!(b.text(), "Hell界!");
        // Hello 世|界! => Hell|界!
        b.insert("o こんにちは 世");
        assert_eq!(b.text(), "Hello こんにちは 世界!");
    }

    #[test]
    fn single_selection() {
        let mut b = Buffer::new();
        b.insert("abXYZcd");
        b.set_cursors(vec![
            Cursor::new(0, 2)
        ]);

        // ab|XYZ|cd
        b.select(0, 0, 0, 3);
        assert_eq!(b.cursors(), &[
            Cursor::Selection(Range::new(0, 2, 0, 5)),
        ]);

        // a|b|XYZcd  =>  a|XYZcd
        b.select(0, 0, 4, 0);
        b.backspace();
        assert_eq!(b.text(), "aXYZcd");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
        ]);

        // a|XYZ|cd  =>  a|cd
        b.select(0, 0, 0, 3);
        b.backspace();
        assert_eq!(b.text(), "acd");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
        ]);

        // ab|  =>  ab|
        // c        |c
        let mut b = Buffer::new();
        b.insert("ab\nc");
        b.set_cursors(vec![
            Cursor::new(0, 2)
        ]);
        b.select(0, 0, 0, 1);
        assert_eq!(b.cursors(), &[
            Cursor::Selection(Range::new(0, 2, 1, 0)),
        ]);
    }

    #[test]
    fn single_selection_including_newlines() {
        // xy|A     xy|z
        // BCD  =>
        // E|z
        let mut b = Buffer::new();
        b.insert("xyA\nBCD\nEz");
        b.set_cursors(vec![
            Cursor::Selection(Range::new(0, 2, 2, 1))
        ]);
        b.backspace();
        assert_eq!(b.text(), "xyz");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 2),
        ]);

        // ab|      abX|c
        // |c   =>
        //
        let mut b = Buffer::new();
        b.insert("ab\nc");
        b.set_cursors(vec![
            Cursor::Selection(Range::new(0, 2, 1, 0))
        ]);
        b.insert("X");
        assert_eq!(b.text(), "abXc");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 3),
        ]);
    }

    #[test]
    fn multi_selections() {
        // ab|XYZ  =>  ab|
        // cd|XYZ  =>  cd|
        // ef|XYZ  =>  ef|
        let mut b = Buffer::new();
        b.insert("abXYZ\ncdXYZ\nefXYZ");
        b.set_cursors(vec![
            Cursor::Selection(Range::new(0, 2, 0, 5)),
            Cursor::Selection(Range::new(1, 2, 1, 5)),
            Cursor::Selection(Range::new(2, 2, 2, 5)),
        ]);
        b.delete();
        assert_eq!(b.text(), "ab\ncd\nef");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 2),
            Cursor::new(1, 2),
            Cursor::new(2, 2),
        ]);

        // ab|XY        ab|cd|ef|g
        // Z|cd|XY  =>
        // Z|ef|XY
        // Z|g
        let mut b = Buffer::new();
        b.insert("abXY\nZcdXY\nZefXY\nZg");
        b.set_cursors(vec![
            Cursor::Selection(Range::new(0, 2, 1, 1)),
            Cursor::Selection(Range::new(1, 3, 2, 1)),
            Cursor::Selection(Range::new(2, 3, 3, 1)),
        ]);
        b.backspace();
        assert_eq!(b.text(), "abcdefg");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 2),
            Cursor::new(0, 4),
            Cursor::new(0, 6),
        ]);
    }

    #[test]
    fn move_to_beginning_of_line() {
        let mut b = Buffer::new();
        // |ab|c|  =>  abc|
        // d|e         de|
        b.insert("abc\nde");
        b.set_cursors(vec![
            Cursor::new(0, 0),
            Cursor::new(0, 2),
            Cursor::new(0, 3),
            Cursor::new(1, 1),
        ]);
        b.move_to_beginning_of_line();
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 0),
            Cursor::new(1, 0),
        ]);
    }

    #[test]
    fn move_to_end_of_line() {
        let mut b = Buffer::new();
        // |ab|c|  =>  abc|
        // d|e         de|
        b.insert("abc\nde");
        b.set_cursors(vec![
            Cursor::new(0, 0),
            Cursor::new(0, 2),
            Cursor::new(0, 3),
            Cursor::new(1, 1),
        ]);
        b.move_to_end_of_line();
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 3),
            Cursor::new(1, 2),
        ]);
    }

    #[test]
    fn select_until_end_of_line() {
        let mut b = Buffer::new();
        // |ab|c|  =>  |abc|
        // d|e         d|e|
        b.insert("abc\nde");
        b.set_cursors(vec![
            Cursor::new(0, 0),
            Cursor::new(0, 2),
            Cursor::new(0, 3),
            Cursor::new(1, 1),
        ]);
        b.select_until_end_of_line();
        assert_eq!(b.cursors(), &[
            Cursor::Selection(Range::new(0, 0, 0, 3)),
            Cursor::Selection(Range::new(1, 1, 1, 2)),
        ]);
    }

    #[test]
    fn set_text() {
        let mut b = Buffer::from_str("");
        b.set_text("abc");
        assert_eq!(b.text(), "abc");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 0),
        ]);

        let mut b = Buffer::from_str("123\n456");
        b.set_text("x");
        assert_eq!(b.text(), "x");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 1),
        ]);
    }

    #[test]
    fn truncate() {
        // abc|XYZ  =>  abc|
        let mut b = Buffer::new();
        b.insert("abcXYZ");
        b.set_cursors(vec![
            Cursor::new(0, 3),
        ]);
        b.truncate();
        assert_eq!(b.text(), "abc");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 3),
        ]);

        // abc|      abc|
        // d|XY  =>  d|
        // |         |
        // |Z        |
        let mut b = Buffer::new();
        b.insert("abc\ndXY\n\nZ");
        b.set_cursors(vec![
            Cursor::new(0, 3),
            Cursor::new(1, 1),
            Cursor::new(2, 0),
            Cursor::new(3, 0),
        ]);
        b.truncate();
        assert_eq!(b.text(), "abc\nd\n\n");
        assert_eq!(b.cursors(), &[
            Cursor::new(0, 3),
            Cursor::new(1, 1),
            Cursor::new(2, 0),
            Cursor::new(3, 0),
        ]);
    }

    #[test]
    fn undo() {
        let mut b = Buffer::new();
        b.redo();
        b.undo();
        assert_eq!(b.text(), "");
        b.insert("abc");
        b.mark_undo_point();
        assert_eq!(b.text(), "abc");
        b.redo(); // Do nothing.
        assert_eq!(b.text(), "abc");
        b.undo();
        assert_eq!(b.text(), "");
        b.redo();
        assert_eq!(b.text(), "abc");
        b.undo();
        assert_eq!(b.text(), "");
        b.undo();
        assert_eq!(b.text(), "");
        b.redo();
        assert_eq!(b.text(), "abc");
        b.redo();
        assert_eq!(b.text(), "abc");

        let mut b = Buffer::new();
        b.insert("abc");
        b.mark_undo_point();
        b.insert("123");
        b.mark_undo_point();
        b.insert("xyz");
        b.mark_undo_point();
        assert_eq!(b.text(), "abc123xyz");
        b.undo();
        assert_eq!(b.text(), "abc123");
        b.undo();
        assert_eq!(b.text(), "abc");
        b.redo();
        assert_eq!(b.text(), "abc123");
        b.redo();
        assert_eq!(b.text(), "abc123xyz");
        b.undo();
        assert_eq!(b.text(), "abc123");
        b.undo();
        assert_eq!(b.text(), "abc");
        b.undo();
        assert_eq!(b.text(), "");
        b.undo();
        assert_eq!(b.text(), "");
    }

    #[test]
    fn current_word() {
        // hello wor|ld from rust
        let mut b = Buffer::from_str("hello world from rust");
        b.set_cursors(vec![Cursor::new(0, 9)]);
        assert_eq!(b.current_word(), Some("world".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 6, 0, 11)));

        // hello |world from rust
        b.set_cursors(vec![Cursor::new(0, 6)]);
        assert_eq!(b.current_word(), Some("world".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 6, 0, 11)));

        // hello world| from rust
        b.set_cursors(vec![Cursor::new(0, 11)]);
        assert_eq!(b.current_word(), Some("world".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 6, 0, 11)));

        // a b| c
        let mut b = Buffer::from_str("a b c");
        b.set_cursors(vec![Cursor::new(0, 3)]);
        assert_eq!(b.current_word(), Some("b".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 2, 0, 3)));

        // |a b c
        let mut b = Buffer::from_str("a b c");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        assert_eq!(b.current_word(), Some("a".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 0, 0, 1)));

        // a | b
        let mut b = Buffer::from_str("a  b");
        b.set_cursors(vec![Cursor::new(0, 2)]);
        assert_eq!(b.current_word(), None);
        assert_eq!(b.current_word_range(), None);

        // |
        let mut b = Buffer::from_str("");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        assert_eq!(b.current_word(), None);
        assert_eq!(b.current_word_range(), None);
    }

    #[test]
    fn move_to_prev_word() {
        // abc 123|  =>  abc |123
        let mut b = Buffer::from_str("abc 123");
        b.set_cursors(vec![Cursor::new(0, 7)]);
        b.move_to_prev_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 4)]);

        // abc |123  =>  |abc 123
        b.move_to_prev_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);
        b.move_to_prev_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        // abc 123  xy|z  =>  abc 123  |xyz  =>  abc |123  xyz  => |abc 123  xyz
        let mut b = Buffer::from_str("abc 123  xyz");
        b.set_cursors(vec![Cursor::new(0, 11)]);
        b.move_to_prev_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 9)]);
        b.move_to_prev_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 4)]);
        b.move_to_prev_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        // a  =>  a
        // |b     |b
        let mut b = Buffer::from_str("a\nb");
        b.set_cursors(vec![Cursor::new(1, 0)]);
        b.move_to_prev_word();
        assert_eq!(b.cursors(), &[Cursor::new(1, 0)]);

        // (empty)
        let mut b = Buffer::from_str("");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.move_to_prev_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);
    }

    #[test]
    fn move_to_next_word() {
        // |abc 123  =>  abc| 123
        let mut b = Buffer::from_str("abc 123");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.move_to_next_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 3)]);

        // abc| 123  =>  abc 123|
        b.move_to_next_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 7)]);
        b.move_to_next_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 7)]);

        // a|bc 123  xyz  =>  abc| 123  xyz  =>  abc 123|  xyz  => abc 123  xyz|
        let mut b = Buffer::from_str("abc 123  xyz");
        b.set_cursors(vec![Cursor::new(0, 1)]);
        b.move_to_next_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 3)]);
        b.move_to_next_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 7)]);
        b.move_to_next_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 12)]);

        // |  =>  |
        //
        let mut b = Buffer::from_str("\n");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.move_to_next_word();
        // assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        // (empty)
        let mut b = Buffer::from_str("");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.move_to_next_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        // |a  =>  a|
        // b       b
        let mut b = Buffer::from_str("a\nb");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.move_to_next_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);
        b.move_to_next_word();
        assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);
    }

    #[test]
    fn find() {
        // 012345678901234567890
        // hello rust from rust
        //       ^^^^      ^^^^
        let mut b = Buffer::from_str("hello rust from rust");
        assert_eq!(&b.find("rust"), &[
            Range::new(0, 6, 0, 10),
            Range::new(0, 16, 0, 20),
        ]);

        let mut b = Buffer::from_str("hello rust from rust");
        assert_eq!(&b.find("rrrrr"), &[]);

        let mut b = Buffer::from_str("abXYZ\nXYZab");
        assert_eq!(&b.find("XYZ"), &[
            Range::new(0, 2, 0, 5),
            Range::new(1, 0, 1, 3),
        ]);

        let mut b = Buffer::from_str("abXY\nZab");
        assert_eq!(&b.find("XY\nZ"), &[
            Range::new(0, 2, 1, 1),
        ]);

        let mut b = Buffer::from_str("");
        assert_eq!(&b.find("rrrrr"), &[]);
    }

    #[test]
    fn indent_by_tab() {
        let mut b = Buffer::from_str("abc");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.tab();
        assert_eq!(&b.text(), "    abc");
        assert_eq!(b.cursors(), &[Cursor::new(0, 4)]);

        let mut b = Buffer::from_str("  abc");
        b.set_cursors(vec![Cursor::new(0, 2)]);
        b.tab();
        assert_eq!(&b.text(), "    abc");
        assert_eq!(b.cursors(), &[Cursor::new(0, 4)]);

        let mut b = Buffer::from_str("    abc");
        b.set_cursors(vec![Cursor::new(0, 2)]);
        b.tab();
        assert_eq!(&b.text(), "      abc");
        assert_eq!(b.cursors(), &[Cursor::new(0, 4)]);
    }

    #[test]
    fn indent_size() {
        let b = Buffer::from_str("");
        assert_eq!(b.indent_size(0), 0);

        let b = Buffer::from_str("  X  ");
        assert_eq!(b.indent_size(0), 2);

        let b = Buffer::from_str("         X");
        assert_eq!(b.indent_size(0), 9);
    }

    #[test]
    fn indent_inheriting_prev_line() {
        // Inherit 8 spaces.
        let mut b = Buffer::from_str("        foo();\n");
        b.set_cursors(vec![Cursor::new(1, 0)]);
        b.tab();
        assert_eq!(&b.text(), "        foo();\n        ");
        assert_eq!(b.cursors(), &[Cursor::new(1, 8)]);
    }

    #[test]
    fn indent_by_enter() {
        // Inherit 8 spaces.
        let mut b = Buffer::from_str("        foo();");
        b.set_cursors(vec![Cursor::new(0, 14)]);
        b.insert_char('\n');
        b.tab();
        assert_eq!(&b.text(), "        foo();\n        ");
        assert_eq!(b.cursors(), &[Cursor::new(1, 8)]);
    }

    #[test]
    fn deindent() {
        let mut b = Buffer::from_str("");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.back_tab();
        assert_eq!(&b.text(), "");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        let mut b = Buffer::from_str("    ");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.back_tab();
        assert_eq!(&b.text(), "");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        // len < config.indent_size
        let mut b = Buffer::from_str("  ");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.back_tab();
        assert_eq!(&b.text(), "");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        let mut b = Buffer::from_str("     ");
        b.set_cursors(vec![Cursor::new(0, 5)]);
        b.back_tab();
        assert_eq!(&b.text(), "    ");
        assert_eq!(b.cursors(), &[Cursor::new(0, 4)]);

        // Multiple cursors at the same line.
        let mut b = Buffer::from_str("        ");
        b.set_cursors(vec![Cursor::new(0, 0), Cursor::new(0, 4)]);
        b.back_tab();
        assert_eq!(&b.text(), "    ");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);

        let mut b = Buffer::from_str("        abc");
        b.set_cursors(vec![Cursor::new(0, 8)]);
        b.back_tab();
        assert_eq!(&b.text(), "    abc");
        assert_eq!(b.cursors(), &[Cursor::new(0, 4)]);
        b.back_tab();
        assert_eq!(&b.text(), "abc");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);
    }
}
