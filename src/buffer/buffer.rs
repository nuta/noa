use crate::rope::SearchIter;
use crate::{cursor::*, rope::Rope, Snapshot};

use noa_editorconfig::*;
use noa_langs::Lang;
use std::cmp::{max, min};
use std::collections::HashSet;
use std::fs;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn remove_range(
    buf: &mut Rope,
    range: &Range,
    next_cursor: Option<&Cursor>,
    new_cursors: &mut Vec<Cursor>,
) {
    // Remove the text in the range.
    buf.remove(range);

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
        Some(Cursor::Normal { pos, .. }) if *pos == front => {}
        _ => {
            new_cursors.push(Cursor::new(front.y, front.x));
        }
    }
}

fn guess_lang_from_path(path: &Path) -> &'static Lang {
    let basename = path.file_name().map(|s| s.to_str().unwrap()).unwrap_or("");
    let ext = path.extension().map(|s| s.to_str().unwrap());
    for lang in noa_langs::LANGS {
        if let Some(ext) = ext.as_ref() {
            if lang.filenames.iter().any(|&f| f == basename)
                || lang.extensions.iter().any(|e| e == ext)
            {
                return lang;
            }
        }
    }

    &noa_langs::PLAIN
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(usize);

impl BufferId {
    fn alloc() -> BufferId {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        BufferId(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }
}

pub struct Buffer {
    id: BufferId,
    rope: Rope,
    saved_rope: Rope,
    name: String,
    path: Option<PathBuf>,
    cursors: Vec<Cursor>,
    undo_stack: Vec<Rope>,
    redo_stack: Vec<Rope>,
    lang: &'static Lang,
    config: EditorConfig,
    snapshot_cache: Mutex<(usize, Arc<Snapshot>)>,
}

impl Buffer {
    pub fn new() -> Buffer {
        let rope = Rope::new();
        Buffer {
            id: BufferId::alloc(),
            rope: rope.clone(),
            saved_rope: rope.clone(),
            name: String::new(),
            path: None,
            cursors: vec![Cursor::new(0, 0)],
            undo_stack: vec![rope],
            redo_stack: Vec::new(),
            lang: &noa_langs::PLAIN,
            config: EditorConfig::default(),
            snapshot_cache: Mutex::new((0, Arc::new(Snapshot::empty()))),
        }
    }

    pub fn from_str(text: &str) -> Buffer {
        let mut buf = Buffer::new();
        buf.insert(text);
        buf
    }

    pub fn open_file(path: &Path) -> std::io::Result<Buffer> {
        let file = std::fs::File::open(path)?;
        let lang = guess_lang_from_path(path);
        let rope = Rope::from_reader(file)?;
        let path = path.canonicalize()?;
        let config = EditorConfig::resolve_or_guess(&path);

        let name = match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) => {
                format!("{}/{}", parent.display(), name.to_str().unwrap())
            }
            (None, Some(_)) => format!("{}", path.display()),
            _ => {
                panic!("invalid file path: {}", path.display());
            }
        };

        Ok(Buffer {
            id: BufferId::alloc(),
            rope: rope.clone(),
            saved_rope: rope.clone(),
            name,
            path: Some(path),
            cursors: vec![Cursor::new(0, 0)],
            undo_stack: vec![rope],
            redo_stack: Vec::new(),
            lang,
            config,
            snapshot_cache: Mutex::new((0, Arc::new(Snapshot::empty()))),
        })
    }

    pub fn set_text(&mut self, text: &str) {
        self.rope.clear();
        self.rope.insert(Point::new(0, 0), text);

        let mut pos = match self.cursors[0] {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection(Range { end, .. }) => end,
        };

        pos.y = min(pos.y, self.rope.num_lines().saturating_sub(1));
        pos.x = min(pos.x, self.rope.line_len(pos.y));
        self.cursors = vec![Cursor::from(pos)];
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.rope.len()
    }

    pub fn id(&self) -> BufferId {
        self.id
    }

    pub fn num_lines(&self) -> usize {
        self.rope.num_lines()
    }

    pub fn line_len(&self, y: usize) -> usize {
        self.rope.line_len(y)
    }

    pub fn is_dirty(&self) -> bool {
        self.rope != self.saved_rope
    }

    pub fn lang(&self) -> &'static Lang {
        self.lang
    }

    pub fn set_lang(&mut self, lang: &'static Lang) {
        self.lang = lang;
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

    /// Returns the absolute (canonicalized) path.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the absolute (canonicalized) path for LSP request.
    pub fn path_for_lsp(&self, workspace_dir: &Path) -> Option<PathBuf> {
        match self.path() {
            // Ignore files that're not under the workspace directory.
            Some(path) if path.starts_with(&workspace_dir) => Some(path.to_owned()),
            _ => None,
        }
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn text(&self) -> String {
        self.rope.text()
    }

    pub fn line(&self, line: usize) -> ropey::RopeSlice {
        self.rope.line(line)
    }

    pub fn line_substr(&self, line: usize, start: usize) -> String {
        self.line(line).chars().skip(start).collect()
    }

    pub fn modified_line(&self) -> &Option<usize> {
        self.rope.modified_line()
    }

    pub fn version(&self) -> usize {
        self.rope.version()
    }

    pub fn id_and_version(&self) -> (BufferId, usize) {
        (self.id, self.rope.version())
    }

    pub fn is_virtual_file(&self) -> bool {
        self.path.is_none()
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(path) = &self.path {
            self.rope.save_into_file(path)?;
            self.saved_rope = self.rope.clone();
        }

        Ok(())
    }

    pub fn update_backup(&self, backup_dir: &Path) {
        let backup_path = backup_dir.join(
            self.path
                .as_ref()
                .map(|pathbuf| pathbuf.as_path().strip_prefix("/").unwrap())
                .unwrap_or_else(|| Path::new(&self.name)),
        );

        let parent_dir = backup_path.parent().unwrap();
        if let Err(err) = fs::create_dir_all(parent_dir) {
            error!(
                "failed to create the backup_dir {}: {}",
                parent_dir.display(),
                err
            );
        }

        if let Err(err) = self.rope.save_into_file(&backup_path) {
            error!(
                "failed to create the backup file {}: {}",
                backup_path.display(),
                err
            );
        }
    }

    pub fn take_snapshot(&self) -> Arc<Snapshot> {
        let mut snapshot_cache = self.snapshot_cache.lock().unwrap();
        if snapshot_cache.0 != self.rope.version() {
            snapshot_cache.0 = self.rope.version();
            snapshot_cache.1 = Arc::new(Snapshot::new(&self.rope));
        }

        snapshot_cache.1.clone()
    }

    pub fn cursors(&self) -> &[Cursor] {
        &self.cursors
    }

    pub fn main_cursor(&self) -> &Cursor {
        &self.cursors[0]
    }

    pub fn main_cursor_pos(&self) -> Point {
        match self.main_cursor() {
            Cursor::Normal { pos, .. } => *pos,
            Cursor::Selection(range) => range.end,
        }
    }

    pub fn move_cursor_to(&mut self, pos: Point) {
        let valid = (pos.y == self.num_lines() && pos.x == 0)
            || (pos.y < self.num_lines() && pos.x <= self.line_len(pos.y));
        if !valid {
            return;
        }

        self.set_cursors(vec![Cursor::new(pos.y, pos.x)]);
    }

    pub fn set_cursors(&mut self, cursors: Vec<Cursor>) {
        self.cursors = cursors;
        self.sort_and_merge_cursors();
    }

    pub fn move_cursors(&mut self, up: usize, down: usize, left: usize, right: usize) {
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
                    pos.move_by(&self.rope, up, down, left, right);
                }
                Cursor::Selection(_) => unreachable!(),
            };
        }

        self.sort_and_merge_cursors();
    }

    pub fn move_to_beginning_of_buffer(&mut self) {
        self.move_cursor_to(Point::new(0, 0));
    }

    pub fn move_to_end_of_buffer(&mut self) {
        self.move_cursor_to(Point::new(self.num_lines(), 0));
    }

    pub fn move_to_end_of_line(&mut self) {
        for cursor in &mut self.cursors {
            let y = match cursor {
                Cursor::Normal { pos, .. } => pos.y,
                Cursor::Selection(Range { end, .. }) => end.y,
            };

            *cursor = Cursor::new(y, self.rope.line_len(y));
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

            let new_pos = self.rope.prev_word_end(pos);
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

            let new_pos = self.rope.next_word_end(pos);
            *cursor = Cursor::new(new_pos.y, new_pos.x);
        }

        self.sort_and_merge_cursors();
    }

    pub fn select(&mut self, up: usize, down: usize, left: usize, right: usize) {
        for cursor in &mut self.cursors {
            let (start, mut end) = match cursor {
                Cursor::Normal { pos, .. } => (*pos, *pos),
                Cursor::Selection(Range { start, end }) => (*start, *end),
            };

            end.move_by(&self.rope, up, down, left, right);
            *cursor = Cursor::Selection(Range::from_points(start, end));
        }

        self.sort_and_merge_cursors();
    }

    pub fn select_until_beginning_of_line(&mut self) {
        let mut new_cursors = Vec::new();
        for cursor in self.cursors.iter().rev() {
            let (mut start, mut end) = match cursor {
                Cursor::Normal { pos, .. } => (*pos, *pos),
                Cursor::Selection(Range { start, end }) => (*start, *end),
            };

            if end == start && end.x == 0 && start.y > 0 {
                start.y -= 1;
                start.x = self.line_len(start.y);
            } else {
                end.x = 0;
            }

            new_cursors.push(Cursor::Selection(Range::from_points(start, end)));
        }

        self.set_cursors(new_cursors);
    }

    pub fn select_until_end_of_line(&mut self) {
        let mut new_cursors = Vec::new();
        for cursor in &self.cursors {
            let (start, end) = match cursor {
                Cursor::Normal { pos, .. } if pos.x == self.rope.line_len(pos.y) => {
                    (*pos, Point::new(pos.y + 1, 0))
                }
                Cursor::Normal { pos, .. } => (*pos, Point::new(pos.y, self.rope.line_len(pos.y))),
                Cursor::Selection(Range { start, end }) => {
                    (*start, Point::new(end.y, self.rope.line_len(end.y)))
                }
            };

            new_cursors.push(Cursor::Selection(Range::from_points(start, end)));
        }

        self.set_cursors(new_cursors);
    }

    pub fn select_until_end_of_line_with_newline(&mut self) {
        let mut new_cursors = Vec::new();
        for cursor in &self.cursors {
            let (start, mut end) = match cursor {
                Cursor::Normal { pos, .. } => (*pos, Point::new(pos.y, self.rope.line_len(pos.y))),
                Cursor::Selection(Range { start, end }) => {
                    (*start, Point::new(end.y, self.rope.line_len(end.y)))
                }
            };

            if end.y + 1 < self.num_lines() {
                end.y += 1;
                end.x = 0;
            }

            new_cursors.push(Cursor::Selection(Range::from_points(start, end)));
        }

        self.set_cursors(new_cursors);
    }

    pub fn insert_char(&mut self, ch: char) {
        self.insert(&ch.to_string())
    }

    pub fn insert(&mut self, string: &str) {
        self.rope.reset_modified_line();

        let mut new_cursors = Vec::new();
        let string_count = string.chars().count();
        for c in self.cursors.iter().rev() {
            let (remove, insert_at, end) = match c {
                Cursor::Normal { pos, .. } => (None, *pos, *pos),
                Cursor::Selection(range) => (Some(range), range.front(), range.back()),
            };

            if let Some(remove) = remove {
                self.rope.remove(remove);
            }

            // Handle insertion at the end of file.
            if insert_at.y == self.num_lines() && string != "\n" {
                debug_assert!(insert_at.x == 0);
                self.rope.insert(insert_at, "\n");
            }

            self.rope.insert(insert_at, string);

            let num_newlines_added = string.matches('\n').count();
            let num_newlines_deleted = remove.map(|r| r.back().y - r.front().y).unwrap_or(0);
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

            let x_diff = string
                .rfind('\n')
                .map(|x| string_count - x - 1)
                .unwrap_or(string_count);

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
        self.rope = Rope::new();
        self.cursors = vec![Cursor::new(0, 0)];
    }

    pub fn indent_size(&self, y: usize) -> usize {
        let mut n = 0;
        let line = self.rope.line(y);
        'outer: for c in line.chunks() {
            for ch in c.chars() {
                if !ch.is_ascii_whitespace() {
                    break 'outer;
                }

                n += 1;
            }
        }

        n
    }

    pub fn tab(&mut self) {
        self.rope.reset_modified_line();
        let mut new_cursors = Vec::new();
        for c in self.cursors.iter().rev() {
            let pos = match c {
                Cursor::Normal { pos, .. } => *pos,
                Cursor::Selection(range) => range.front(),
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
                    self.rope.insert_char(pos, ch);
                }
            } else {
                // Not auto indent; the user just wants to input '\t'.
                self.rope.insert_char(pos, '\t');
                x = pos.x + 1;
            }

            new_cursors.push(Cursor::new(pos.y, x));
        }

        self.set_cursors(new_cursors);
    }

    // Decrease indent levels.
    pub fn back_tab(&mut self) {
        self.rope.reset_modified_line();
        let mut new_cursors = Vec::new();
        let mut ys = HashSet::new();
        for c in self.cursors.iter().rev() {
            let pos = match c {
                Cursor::Normal { pos, .. } => *pos,
                Cursor::Selection(range) => range.front(),
            };

            let n = min(
                self.indent_size(pos.y),
                if pos.x % self.config.indent_size == 0 {
                    self.config.indent_size
                } else {
                    pos.x % self.config.indent_size
                },
            );
            if n > 0 && !ys.contains(&pos.y) {
                let start = Point::new(pos.y, 0);
                let end = Point::new(pos.y, n);
                self.rope.remove(&Range::from_points(start, end));
                new_cursors.push(Cursor::new(pos.y, pos.x.saturating_sub(n)));
                ys.insert(pos.y);
            } else {
                new_cursors.push(Cursor::new(pos.y, pos.x));
            }
        }

        self.set_cursors(new_cursors);
    }

    pub fn backspace(&mut self) {
        self.rope.reset_modified_line();

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
                        Point::new(pos.y - 1, self.rope.line_len(pos.y - 1))
                    } else {
                        Point::new(pos.y, pos.x - 1)
                    };

                    Range::from_points(start, *pos)
                }
                Cursor::Selection(range) => range.clone(),
            };

            remove_range(
                &mut self.rope,
                &range,
                iter.peek().copied(),
                &mut new_cursors,
            );
        }

        self.set_cursors(new_cursors);
    }

    pub fn delete(&mut self) {
        self.rope.reset_modified_line();

        let mut new_cursors = Vec::new();
        let mut iter = self.cursors.iter().rev().peekable();
        while let Some(c) = iter.next() {
            // Determine the range to be deleted.
            let range = match c {
                Cursor::Normal { pos, .. } => {
                    let max_y = self.rope.num_lines();
                    let max_x = self.rope.line_len(pos.y);
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
                Cursor::Selection(range) => range.clone(),
            };

            remove_range(
                &mut self.rope,
                &range,
                iter.peek().copied(),
                &mut new_cursors,
            );
        }

        self.set_cursors(new_cursors);
    }

    pub fn truncate(&mut self) {
        self.rope.reset_modified_line();

        self.select_until_end_of_line();
        self.delete();
    }

    pub fn truncate_reverse(&mut self) {
        self.rope.reset_modified_line();

        self.select_until_beginning_of_line();
        self.delete();
    }

    pub fn transform_selections_with<F>(&mut self, mut transform: F)
    where
        F: FnMut(&Range, &str) -> String,
    {
        self.rope.reset_modified_line();

        let mut new_cursors = Vec::new();
        let mut iter = self.cursors.iter().rev().peekable();
        while let Some(c) = iter.next() {
            // Determine the range to be deleted.
            let range = match c {
                Cursor::Normal { .. } => {
                    continue;
                }
                Cursor::Selection(range) => range.clone(),
            };

            let old_text = self.rope.sub_str(&range).to_string();
            let new_text = transform(&range, &old_text);

            remove_range(
                &mut self.rope,
                &range,
                iter.peek().copied(),
                &mut new_cursors,
            );
            self.rope.insert(range.front(), &new_text);
        }

        if !new_cursors.is_empty() {
            self.set_cursors(new_cursors);
        }
    }

    pub fn mark_undo_point(&mut self) {
        match self.undo_stack.last() {
            Some(rope) if *rope == self.rope => {
                // The buffer is not modified.
                return;
            }
            _ => {}
        }

        self.undo_stack.push(self.rope.clone());
    }

    pub fn undo(&mut self) {
        if self.undo_stack.len() == 1 && self.rope.is_empty() {
            return;
        }

        if let Some(top) = self.undo_stack.last() {
            if *top == self.rope {
                self.undo_stack.pop();
            }
        }

        if let Some(buf) = self.undo_stack.pop() {
            self.redo_stack.push(self.rope.clone());
            self.rope = buf;
        }
    }

    pub fn redo(&mut self) {
        if let Some(buf) = self.redo_stack.pop() {
            self.undo_stack.push(self.rope.clone());
            self.rope = buf;
        }
    }

    pub fn select_all(&mut self) {
        let range = Range::new(0, 0, self.num_lines(), 0);
        self.set_cursors(vec![Cursor::Selection(range)]);
    }

    pub fn cut_selection(&mut self) -> String {
        let text = self.copy_selection();
        self.backspace();
        text
    }

    pub fn copy_selection(&mut self) -> String {
        let mut text = String::new();
        for (i, c) in self.cursors.iter().enumerate() {
            let range = match c {
                Cursor::Selection(range) => range,
                _ => continue,
            };

            if i > 0 {
                text.push('\n');
            }
            for chunk in self.rope.sub_str(range).chunks() {
                text += chunk;
            }
        }

        text
    }

    pub fn paste(&mut self, text: &str) {
        if self.cursors.len() == 1 {
            self.insert(text);
        } else {
            let cursors = self.cursors.clone();
            let mut new_cursors = Vec::with_capacity(self.cursors.len());
            let lines: Vec<&str> = text.split('\n').collect();
            for (c, line) in cursors.iter().rev().zip(lines.iter().rev()) {
                self.cursors = vec![c.clone()];
                self.insert(line);
                new_cursors.push(Cursor::new(c.front().y, c.front().x + line.chars().count()));
            }

            self.set_cursors(new_cursors);
        }
    }

    /// Sorts the cursors and removes overlapped ones. Don't forget to call this
    /// method when you made a change.
    fn sort_and_merge_cursors(&mut self) {
        debug_assert!(!self.cursors.is_empty());

        self.cursors.sort();
        let duplicated = self.cursors.iter().enumerate().map(|(i, c)| match c {
            Cursor::Normal { pos, .. } => (&self.cursors[..i]).iter().any(|other| match other {
                Cursor::Normal { pos: ref other, .. } => *pos == *other,
                _ => unreachable!(),
            }),
            Cursor::Selection(range) => (&self.cursors[..i]).iter().any(|other| match other {
                Cursor::Selection(ref other) => range.overlaps_with(other),
                _ => unreachable!(),
            }),
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

        self.rope.word_at(pos).map(|(_, word)| word)
    }

    pub fn current_word_range(&self) -> Option<Range> {
        let pos = match &self.cursors[0] {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection(Range { start, .. }) => start,
        };

        self.rope.word_at(pos).map(|(range, _)| range)
    }

    pub fn prev_word_range(&self) -> Option<Range> {
        let pos = match &self.cursors[0] {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection(Range { start, .. }) => start,
        };

        self.rope.prev_word_at(pos)
    }

    pub fn prev_word_ranges(&self) -> Vec<Range> {
        let mut ranges = Vec::with_capacity(self.cursors.len());
        for c in &self.cursors {
            let pos = match c {
                Cursor::Normal { pos, .. } => pos,
                Cursor::Selection(Range { start, .. }) => start,
            };

            let range = self
                .rope
                .prev_word_at(pos)
                .unwrap_or_else(|| Range::from_points(*pos, *pos));

            ranges.push(range);
        }

        ranges
    }

    pub fn current_line_range(&self) -> Range {
        let cursor = self.main_cursor_pos();
        Range::new(cursor.y, 0, cursor.y, self.line_len(cursor.y))
    }

    pub fn select_by_ranges(&mut self, selections: &[Range]) {
        self.cursors.clear();
        for selection in selections {
            self.cursors.push(Cursor::Selection(selection.clone()));
        }

        self.sort_and_merge_cursors();
    }

    pub fn add_cursor(&mut self, cursor: Cursor) {
        self.cursors.push(cursor);
        self.sort_and_merge_cursors();
    }

    pub fn add_cursor_above(&mut self) {
        let top = self.cursors[0].anchor();
        if top.y > 0 {
            let y = top.y - 1;
            self.add_cursor(Cursor::new(y, min(self.line_len(y), top.x)));
        }
    }

    pub fn add_cursor_below(&mut self) {
        let bottom = self.cursors[self.cursors.len() - 1].anchor();
        if bottom.y < self.num_lines() {
            let y = bottom.y + 1;
            let x = if bottom.y == self.num_lines() {
                0
            } else {
                min(self.line_len(y), bottom.x)
            };
            self.add_cursor(Cursor::new(y, x));
        }
    }

    pub fn move_current_line_above(&mut self) {
        let old_cursors = self.cursors.clone();
        self.move_to_beginning_of_line();
        self.select_until_end_of_line_with_newline();
        let text = self.cut_selection();
        self.move_cursors(1, 0, 0, 0);
        self.paste(&text);
        if !text.ends_with('\n') {
            self.insert_char('\n');
            self.move_to_end_of_line();
            self.delete();
        }

        // Try to restore cursors' x.
        self.set_cursors(old_cursors);
        self.move_cursors(1, 0, 0, 0);
    }

    pub fn move_current_line_below(&mut self) {
        let old_cursors = self.cursors.clone();
        self.move_to_beginning_of_line();
        self.select_until_end_of_line_with_newline();
        let text = self.cut_selection();
        self.move_cursors(0, 1, 0, 0);
        self.paste(&text);
        if !text.ends_with('\n') {
            self.insert_char('\n');
            self.move_to_end_of_line();
            self.delete();
        }

        // Try to restore cursors' x.
        self.set_cursors(old_cursors);
        self.move_cursors(0, 1, 0, 0);
    }

    pub fn duplicate_line_above(&mut self) {
        let old_cursors = self.cursors.clone();
        self.move_to_beginning_of_line();
        self.select_until_end_of_line_with_newline();

        let text = self.copy_selection();
        self.move_to_beginning_of_line();

        self.paste(&text);
        if !text.ends_with('\n') {
            self.insert_char('\n');
            self.move_to_end_of_line();
            self.delete();
        }

        // Try to restore cursors' x.
        self.set_cursors(old_cursors);
    }

    pub fn duplicate_line_below(&mut self) {
        let old_cursors = self.cursors.clone();
        self.move_to_beginning_of_line();
        self.select_until_end_of_line_with_newline();

        let text = self.copy_selection();
        self.move_to_beginning_of_line();

        self.paste(&text);
        if !text.ends_with('\n') {
            self.insert_char('\n');
            self.move_to_end_of_line();
            self.delete();
        }

        // Try to restore cursors' x.
        self.set_cursors(old_cursors);
        self.move_cursors(0, 1, 0, 0);
    }

    pub fn find_prev(&self, needle: &str, before: Option<Point>) -> Option<Range> {
        self.rope.find_prev(needle, before)
    }

    pub fn find_next(&self, needle: &str, after: Option<Point>) -> Option<Range> {
        self.rope.find_next(needle, after)
    }

    pub fn find_prev_by_regex(
        &self,
        pattern: &str,
        after: Option<Point>,
    ) -> Result<Option<Range>, regex::Error> {
        self.rope.find_prev_by_regex(pattern, after)
    }

    pub fn find_next_by_regex(
        &self,
        pattern: &str,
        after: Option<Point>,
    ) -> Result<Option<Range>, regex::Error> {
        self.rope.find_next_by_regex(pattern, after)
    }

    pub fn find_all<'a>(&'a self, needle: &str, after: Option<Point>) -> SearchIter<'a> {
        self.rope.find_all(needle, after)
    }

    pub fn find_all_by_regex<'a>(
        &'a self,
        pattern: &str,
        after: Option<Point>,
    ) -> Result<SearchIter<'a>, regex::Error> {
        self.rope.find_all_by_regex(pattern, after)
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
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 6), Cursor::new(1, 4), Cursor::new(2, 3),]
        );

        // abc123[
        // ]|
        // d123[
        // ]|e
        // 123[
        // ]|xyz
        b.insert("[\n]");
        assert_eq!(b.text(), "abc123[\n]\nd123[\n]e\n123[\n]xyz");
        assert_eq!(
            b.cursors(),
            &[Cursor::new(1, 1), Cursor::new(3, 1), Cursor::new(5, 1),]
        );
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
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 2), Cursor::new(1, 2), Cursor::new(2, 2),]
        );

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
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 2), Cursor::new(1, 0), Cursor::new(2, 1),]
        );

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
        assert_eq!(b.cursors(), &[Cursor::new(0, 3),]);

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
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 0), Cursor::new(0, 2), Cursor::new(1, 2),]
        );

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
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 1), Cursor::new(0, 4), Cursor::new(0, 6),]
        );

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
        assert_eq!(b.cursors(), &[Cursor::new(0, 1), Cursor::new(0, 4),]);

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
        assert_eq!(b.cursors(), &[Cursor::new(0, 1), Cursor::new(0, 4),]);
    }

    #[test]
    fn delete_on_multi_cursors() {
        // a|Xbc|Yd
        let mut b = Buffer::new();
        b.insert("aXbcYd");
        b.set_cursors(vec![Cursor::new(0, 1), Cursor::new(0, 4)]);
        b.delete();
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1), Cursor::new(0, 3),]);

        // a|b|
        let mut b = Buffer::new();
        b.insert("ab");
        b.set_cursors(vec![Cursor::new(0, 1), Cursor::new(0, 2)]);
        b.delete();
        assert_eq!(b.text(), "a");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1),]);

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
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 1), Cursor::new(1, 1), Cursor::new(2, 1),]
        );

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
        assert_eq!(
            b.cursors(),
            &[
                Cursor::new(0, 1),
                Cursor::new(0, 2),
                Cursor::new(1, 1),
                Cursor::new(2, 1),
            ]
        );

        // ab|
        // cde|
        let mut b = Buffer::new();
        b.insert("ab\ncde");
        b.set_cursors(vec![Cursor::new(0, 2), Cursor::new(1, 3)]);
        b.delete();
        assert_eq!(b.text(), "abcde");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2), Cursor::new(0, 5),]);

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
        assert_eq!(b.cursors(), &[Cursor::new(0, 3), Cursor::new(1, 3),]);

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
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 3), Cursor::new(0, 4), Cursor::new(0, 5),]
        );
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
    fn multibyte_characters_regression1() {
        let mut b = Buffer::new();
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.insert_char('a');
        b.insert_char('あ');
        b.insert_char('!');
        assert_eq!(b.text(), "aあ!");
    }

    #[test]
    fn single_selection() {
        let mut b = Buffer::new();
        b.insert("abXYZcd");
        b.set_cursors(vec![Cursor::new(0, 2)]);

        // ab|XYZ|cd
        b.select(0, 0, 0, 3);
        assert_eq!(b.cursors(), &[Cursor::Selection(Range::new(0, 2, 0, 5)),]);

        // a|b|XYZcd  =>  a|XYZcd
        b.select(0, 0, 4, 0);
        b.backspace();
        assert_eq!(b.text(), "aXYZcd");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1),]);

        // a|XYZ|cd  =>  a|cd
        b.select(0, 0, 0, 3);
        b.backspace();
        assert_eq!(b.text(), "acd");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1),]);

        // ab|  =>  ab|
        // c        |c
        let mut b = Buffer::new();
        b.insert("ab\nc");
        b.set_cursors(vec![Cursor::new(0, 2)]);
        b.select(0, 0, 0, 1);
        assert_eq!(b.cursors(), &[Cursor::Selection(Range::new(0, 2, 1, 0)),]);
    }

    #[test]
    fn single_selection_including_newlines() {
        // xy|A     xy|z
        // BCD  =>
        // E|z
        let mut b = Buffer::new();
        b.insert("xyA\nBCD\nEz");
        b.set_cursors(vec![Cursor::Selection(Range::new(0, 2, 2, 1))]);
        b.backspace();
        assert_eq!(b.text(), "xyz");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2),]);

        // ab|      abX|c
        // |c   =>
        //
        let mut b = Buffer::new();
        b.insert("ab\nc");
        b.set_cursors(vec![Cursor::Selection(Range::new(0, 2, 1, 0))]);
        b.insert("X");
        assert_eq!(b.text(), "abXc");
        assert_eq!(b.cursors(), &[Cursor::new(0, 3),]);
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
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 2), Cursor::new(1, 2), Cursor::new(2, 2),]
        );

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
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 2), Cursor::new(0, 4), Cursor::new(0, 6),]
        );
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
        assert_eq!(b.cursors(), &[Cursor::new(0, 0), Cursor::new(1, 0),]);
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
        assert_eq!(b.cursors(), &[Cursor::new(0, 3), Cursor::new(1, 2),]);
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
            Cursor::new(1, 1),
        ]);
        b.select_until_end_of_line();
        assert_eq!(
            b.cursors(),
            &[
                Cursor::Selection(Range::new(0, 0, 0, 3)),
                Cursor::Selection(Range::new(1, 1, 1, 2)),
            ]
        );
    }

    #[test]
    fn select_until_end_of_line_with_newline() {
        let mut b = Buffer::new();
        // |ab|c|  =>  |abc|
        // d|e         d|e|
        b.insert("abc\nde");
        b.set_cursors(vec![
            Cursor::new(0, 0),
            Cursor::new(0, 2),
            Cursor::new(1, 1),
        ]);
        b.select_until_end_of_line_with_newline();
        assert_eq!(
            b.cursors(),
            &[
                Cursor::Selection(Range::new(0, 0, 1, 0)),
                Cursor::Selection(Range::new(1, 1, 1, 2)),
            ]
        );

        let mut b = Buffer::new();
        b.insert("\n\n");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        b.select_until_end_of_line_with_newline();
        assert_eq!(b.cursors(), &[Cursor::Selection(Range::new(0, 0, 1, 0)),]);
    }

    #[test]
    fn set_text() {
        let mut b = Buffer::from_str("");
        b.set_text("abc");
        assert_eq!(b.text(), "abc");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0),]);

        let mut b = Buffer::from_str("123\n456");
        b.set_text("x");
        assert_eq!(b.text(), "x");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1),]);
    }

    #[test]
    fn truncate() {
        // abc|XYZ  =>  abc|
        let mut b = Buffer::new();
        b.insert("abcXYZ");
        b.set_cursors(vec![Cursor::new(0, 3)]);
        b.truncate();
        assert_eq!(b.text(), "abc");
        assert_eq!(b.cursors(), &[Cursor::new(0, 3),]);

        // abc|      abc|xyz
        // xyz  =>
        let mut b = Buffer::new();
        b.insert("abc\nxyz");
        b.set_cursors(vec![Cursor::new(0, 3)]);
        b.truncate();
        assert_eq!(b.text(), "abcxyz");
        assert_eq!(b.cursors(), &[Cursor::new(0, 3),]);
    }

    #[test]
    fn truncate_reverse() {
        // abc|XYZ  =>  abc|
        let mut b = Buffer::new();
        b.insert("abcXYZ");
        b.set_cursors(vec![Cursor::new(0, 3)]);
        b.truncate_reverse();
        assert_eq!(b.text(), "XYZ");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0),]);

        // abc       abc|xyz
        // |xyz  =>
        let mut b = Buffer::new();
        b.insert("abc\nxyz");
        b.set_cursors(vec![Cursor::new(1, 0)]);
        b.truncate_reverse();
        assert_eq!(b.text(), "abcxyz");
        assert_eq!(b.cursors(), &[Cursor::new(0, 3),]);
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
    fn prev_word_range() {
        // abc|
        let mut b = Buffer::from_str("abc");
        b.set_cursors(vec![Cursor::new(0, 3)]);
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 0, 0, 3)));

        // abc xyz|
        let mut b = Buffer::from_str("abc xyz");
        b.set_cursors(vec![Cursor::new(0, 7)]);
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 4, 0, 7)));

        // abc|xyz
        let mut b = Buffer::from_str("abcxyz");
        b.set_cursors(vec![Cursor::new(0, 3)]);
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 0, 0, 3)));

        // abc xyz;|
        let mut b = Buffer::from_str("abc xyz;");
        b.set_cursors(vec![Cursor::new(0, 8)]);
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 7, 0, 8)));

        // abc !@#|
        let mut b = Buffer::from_str("abc !@#");
        b.set_cursors(vec![Cursor::new(0, 7)]);
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 3, 0, 7)));

        // ____abc
        let mut b = Buffer::from_str("    abc");
        b.set_cursors(vec![Cursor::new(0, 4)]);
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 0, 0, 4)));

        // (empty)
        let mut b = Buffer::from_str("");
        b.set_cursors(vec![Cursor::new(0, 0)]);
        assert_eq!(b.prev_word_range(), None);

        // abc
        // |
        let mut b = Buffer::from_str("abc\n");
        b.set_cursors(vec![Cursor::new(1, 0)]);
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 3, 1, 0)));
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

    #[test]
    fn copy_and_paste() {
        let mut b = Buffer::from_str("aXYZb");
        b.set_cursors(vec![Cursor::Selection(Range::new(0, 1, 0, 4))]);
        assert_eq!(b.copy_selection(), "XYZ");
        assert_eq!(b.text(), "aXYZb");
        assert_eq!(b.cut_selection(), "XYZ");
        assert_eq!(b.text(), "ab");
        b.paste("123");
        assert_eq!(b.text(), "a123b");
    }

    #[test]
    fn move_current_line_above() {
        // A
        // 123
        // xy
        let mut b = Buffer::from_str("A\n123\nxy");
        b.set_cursors(vec![Cursor::new(2, 1)]);

        b.move_current_line_above();
        assert_eq!(b.text(), "A\nxy\n123");
        assert_eq!(b.cursors(), &[Cursor::new(1, 1)]);

        b.move_current_line_above();
        assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);
        assert_eq!(b.text(), "xy\nA\n123");

        // No changes.
        b.move_current_line_above();
        assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);
        assert_eq!(b.text(), "xy\nA\n123");
    }

    #[test]
    fn move_current_line_above_empty_line() {
        //
        //
        // ABC
        //
        let mut b = Buffer::from_str("\n\nABC\n\n");
        b.set_cursors(vec![Cursor::new(3, 0)]);

        b.move_current_line_above();
        assert_eq!(b.text(), "\n\n\nABC\n");
        assert_eq!(b.cursors(), &[Cursor::new(2, 0)]);

        b.move_current_line_above();
        assert_eq!(b.text(), "\n\n\nABC\n");
        assert_eq!(b.cursors(), &[Cursor::new(1, 0)]);

        b.move_current_line_above();
        assert_eq!(b.text(), "\n\n\nABC\n");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);
    }
}
