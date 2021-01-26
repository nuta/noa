use crate::editorconfig::{EditorConfig, IndentStyle};
use crate::language::{guess_language, Language};
use crate::rope::*;
use std::cmp::{max, min};

use std::fs;

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct TopLeft {
    pub y: usize,
    pub x: usize,
}

impl TopLeft {
    pub fn new(y: usize, x: usize) -> TopLeft {
        TopLeft { y, x }
    }
}

pub struct Buffer {
    rope: Rope,
    last_saved_rope: Rope,
    name: String,
    file: Option<PathBuf>,
    cursor: Cursor,
    top_left: TopLeft,
    undo_stack: Vec<Rope>,
    redo_stack: Vec<Rope>,
    language: &'static Language,
    config: EditorConfig,
}

impl Buffer {
    pub fn new() -> Buffer {
        let language = &crate::language::PLAIN;
        let rope = Rope::new();
        Buffer {
            rope: rope.clone(),
            last_saved_rope: rope.clone(),
            name: String::new(),
            file: None,
            cursor: Cursor::new(0, 0),
            top_left: TopLeft::new(0, 0),
            undo_stack: vec![rope],
            redo_stack: Vec::new(),
            language,
            config: EditorConfig::default(),
        }
    }

    #[cfg(test)]
    pub fn from_str(text: &str) -> Buffer {
        let mut buf = Buffer::new();
        buf.insert(text);
        buf
    }

    pub fn open_file(path: &Path) -> std::io::Result<Buffer> {
        let language = guess_language(path);
        let file = std::fs::File::open(path)?;
        let rope = Rope::from_reader(file)?;

        Ok(Buffer {
            rope: rope.clone(),
            last_saved_rope: rope.clone(),
            name: String::new(),
            file: Some(path.canonicalize()?),
            cursor: Cursor::new(0, 0),
            top_left: TopLeft::new(0, 0),
            undo_stack: vec![rope],
            redo_stack: Vec::new(),
            language,
            config: EditorConfig::resolve(path),
        })
    }

    #[cfg(test)]
    pub fn set_text(&mut self, text: &str) {
        self.rope.clear();
        self.rope.insert(&Point::new(0, 0), text);

        let mut pos = match self.cursor {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection {
                range: Range { end, .. },
                ..
            } => end,
        };

        pos.y = min(pos.y, self.rope.num_lines().saturating_sub(1));
        pos.x = min(pos.x, self.rope.line_len(pos.y));
        self.cursor = Cursor::from_point(&pos);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.rope.len()
    }

    pub fn num_lines(&self) -> usize {
        self.rope.num_lines()
    }

    pub fn line_len(&self, y: usize) -> usize {
        self.rope.line_len(y)
    }

    pub fn is_dirty(&self) -> bool {
        self.rope != self.last_saved_rope
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name<T: Into<String>>(&mut self, name: T) {
        self.name = name.into();
    }

    pub fn path(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    pub fn config(&self) -> &EditorConfig {
        &self.config
    }

    #[cfg(test)]
    pub fn text(&self) -> String {
        self.rope.text()
    }

    pub fn line(&self, line: usize) -> ropey::RopeSlice {
        self.rope.line(line)
    }

    /// Returns `(chunk_idx, char_idx_in_chunk)`.
    pub fn line_substr_chunk(&self, line: usize, start: usize) -> Option<(usize, usize)> {
        let mut remaining = start;
        for (chunk_i, chunk) in self.line(line).chunks().enumerate() {
            for (char_i, _) in chunk.char_indices() {
                if remaining == 0 {
                    return Some((chunk_i, char_i));
                }
                remaining -= 1;
            }
        }

        None
    }

    pub fn line_substr(&self, line: usize, start: usize) -> String {
        let mut s = String::new();
        let mut iter = self.line(line).chunks();
        if start > 0 {
            let mut skip = start;
            'outer: while let Some(chunk) = iter.next() {
                for (i, _) in chunk.char_indices() {
                    if skip == 0 {
                        s += &chunk[i..];
                        break 'outer;
                    }
                    skip -= 1;
                }
            }
        }

        for chunk in iter {
            s += chunk;
        }

        s
    }

    #[cfg(test)]
    pub fn set_language(&mut self, language: &'static Language) {
        self.language = language;
    }

    pub fn is_virtual_file(&self) -> bool {
        self.file.is_none()
    }

    pub fn save(&mut self, backup_dir: &Path) -> std::io::Result<()> {
        if let Some(path) = &self.file {
            if path.exists() {
                let filename = path.to_str().unwrap().replace('/', ".");
                fs::create_dir_all(backup_dir)?;
                fs::copy(path, backup_dir.join(&filename)).ok();
            }
            self.rope.save_into_file(path)?;
            self.last_saved_rope = self.rope.clone();
        }

        Ok(())
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub fn main_cursor_pos(&self) -> &Point {
        match &self.cursor {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection { range, .. } => &range.end,
        }
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
    }

    pub fn top_left(&self) -> &TopLeft {
        &self.top_left
    }

    pub fn adjust_top_left(&mut self, rows: usize, cols: usize) {
        let pos = match &self.cursor {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection {
                range: Range { end, .. },
                ..
            } => end,
        };

        // Scroll Up.
        if pos.y < self.top_left.y {
            self.top_left.y = pos.y;
        }

        // Scroll Down.
        if pos.y >= self.top_left.y + rows {
            self.top_left.y = pos.y - rows + 1;
        }

        self.top_left.y = min(self.top_left.y, self.num_lines().saturating_sub(rows / 2));

        // TODO:
        // // Scroll Right.
        // if pos.x >= self.top_left.x + cols {
        //     self.top_left.x = pos.x - cols + 1;
        // }

        // // Scroll Left.
        // if pos.x < self.top_left.x {
        //     self.top_left.x = pos.x;
        // }
    }

    pub fn goto(&mut self, y: usize, x: usize) {
        let clamped_y = min(y, self.num_lines());
        let clamped_x = min(x, self.line_len(clamped_y));
        self.set_cursor(Cursor::new(clamped_y, clamped_x));
    }

    pub fn scroll_up(&mut self, y_diff: usize) {
        self.move_cursor(y_diff, 0, 0, 0);
        self.top_left.y = self.top_left.y.saturating_sub(y_diff);
    }

    pub fn scroll_down(&mut self, y_diff: usize) {
        self.move_cursor(0, y_diff, 0, 0);
        self.top_left.y = min(self.num_lines(), self.top_left.y + y_diff);
    }

    pub fn centering(&mut self, rows: usize) {
        if let Cursor::Normal { pos, .. } = self.cursor() {
            self.top_left.y = pos.y.saturating_sub(rows / 2);
        }
    }

    fn cancel_selection(&mut self) {
        match &mut self.cursor {
            Cursor::Normal { .. } => {}
            Cursor::Selection { range, .. } => {
                self.cursor = Cursor::from_point(&range.end);
            }
        }
    }

    pub fn move_cursor(&mut self, up: usize, down: usize, left: usize, right: usize) {
        self.cancel_selection();
        match &mut self.cursor {
            Cursor::Normal { pos, logical_x } => {
                pos.move_by(&self.rope, up, down, left, right);
                *logical_x = pos.x;
            }
            Cursor::Selection { .. } => unreachable!(),
        }
    }

    fn width_in_display(&self, y: usize, x_start: usize, x_end: usize) -> usize {
        assert!(x_start <= x_end);
        assert!(x_end <= self.line_len(y));
        if y == self.num_lines() {
            return 0;
        }

        use unicode_width::UnicodeWidthStr;
        let rest = self.line_substr(y, x_start);
        let end_i = rest
            .char_indices()
            .nth(x_end - x_start)
            .map(|(i, _)| i)
            .unwrap_or_else(|| rest.len());
        UnicodeWidthStr::width_cjk(&rest[..end_i])
    }

    pub fn move_cursor_with_line_wrap(&mut self, cols: usize, up: usize, down: usize) {
        self.cancel_selection();

        let (mut pos, logical_x) = match &self.cursor {
            Cursor::Normal { pos, logical_x } => (*pos, *logical_x),
            Cursor::Selection { range, logical_x } => (range.end, *logical_x),
        };

        for _ in 0..down {
            let prev_x = pos.x;
            let _prev_y = pos.y;
            let prefix_width = self.width_in_display(pos.y, 0, pos.x);
            let from_left = prefix_width % (cols + 1);
            // Move at the same display column in the next line in the display.
            'outer1: loop {
                if pos.x >= self.line_len(pos.y) {
                    if pos.y == self.num_lines() || pos.x - prev_x > cols - from_left {
                        break;
                    }

                    // Move at the same display column in the next line in the buffer.
                    pos.y += 1;
                    pos.x = 0;
                    let prefix_width =
                        self.width_in_display(pos.y, 0, min(self.line_len(pos.y), logical_x));
                    let from_left = prefix_width % (cols + 1);
                    loop {
                        if pos.x >= self.line_len(pos.y)
                            || self.width_in_display(pos.y, 0, pos.x) >= from_left
                        {
                            break 'outer1;
                        }

                        pos.x += 1;
                    }
                }

                if self.width_in_display(pos.y, prev_x, pos.x) >= cols {
                    break;
                }

                pos.x += 1;
            }
        }

        for _ in 0..up {
            let prev_x = pos.x;
            let _prev_y = pos.y;
            let prefix_width = self.width_in_display(pos.y, 0, pos.x);
            let from_left = prefix_width % (cols + 1);
            // Move at the same display column in the previous line in the display.
            'outer2: loop {
                if self.width_in_display(pos.y, pos.x, prev_x) >= cols {
                    break;
                }

                if pos.x == 0 {
                    if pos.y == 0 {
                        pos.x = prev_x;
                        break;
                    }

                    // Move at the same display column in the previous line in the buffer.
                    let prev_line_len = self.line_len(pos.y - 1);
                    pos.y -= 1;
                    pos.x = prev_line_len;
                    let prev_line_width =
                        self.width_in_display(pos.y, 0, prev_line_len) % (cols + 1);
                    let logical_x_width = if logical_x <= prev_line_len {
                        self.width_in_display(pos.y, 0, logical_x) % (cols + 1)
                    } else {
                        0
                    };
                    if prev_line_width <= from_left {
                        break;
                    }
                    loop {
                        if pos.x == 0
                            || self.width_in_display(pos.y, pos.x, prev_line_len)
                                >= prev_line_width - max(from_left, logical_x_width)
                        {
                            break 'outer2;
                        }

                        pos.x -= 1;
                    }
                }

                pos.x -= 1;
            }
        }

        match &mut self.cursor {
            Cursor::Normal {
                pos: current_pos, ..
            } => *current_pos = pos,
            Cursor::Selection { range, .. } => range.end = pos,
        };
    }

    pub fn move_to_end_of_line(&mut self) {
        let y = match self.cursor() {
            Cursor::Normal { pos, .. } => pos.y,
            Cursor::Selection {
                range: Range { end, .. },
                ..
            } => end.y,
        };

        self.set_cursor(Cursor::new(y, self.rope.line_len(y)));
    }

    pub fn move_to_beginning_of_line(&mut self) {
        let y = match self.cursor() {
            Cursor::Normal { pos, .. } => pos.y,
            Cursor::Selection {
                range: Range { end, .. },
                ..
            } => end.y,
        };

        self.set_cursor(Cursor::new(y, 0));
    }

    pub fn move_to_prev_word(&mut self) {
        let pos = match self.cursor() {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection {
                range: Range { start, .. },
                ..
            } => start,
        };

        let new_pos = self.rope.prev_word_end(&pos);
        self.set_cursor(Cursor::new(new_pos.y, new_pos.x));
    }

    pub fn move_to_next_word(&mut self) {
        let pos = match &mut self.cursor {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection {
                range: Range { start, .. },
                ..
            } => start,
        };

        let new_pos = self.rope.next_word_end(&pos);
        self.set_cursor(Cursor::new(new_pos.y, new_pos.x));
    }

    pub fn move_to_prev_block(&mut self) {
        let y_start = self.main_cursor_pos().y;
        for y in (1..y_start).rev() {
            if self.indent_size(y - 1) == self.line_len(y - 1) {
                self.set_cursor(Cursor::new(y, 0));
                return;
            }
        }

        // Reached the beginning of the file.
        self.set_cursor(Cursor::new(0, 0));
    }

    pub fn move_to_next_block(&mut self) {
        let y_start = self.main_cursor_pos().y;
        for y in y_start..self.num_lines() {
            if self.indent_size(y) == self.line_len(y) {
                self.set_cursor(Cursor::new(min(y + 1, self.num_lines()), 0));
                return;
            }
        }

        // Reached the EOF.
        self.set_cursor(Cursor::new(self.num_lines(), 0));
    }

    pub fn select(&mut self, up: usize, down: usize, left: usize, right: usize) {
        let (start, mut end) = match &mut self.cursor {
            Cursor::Normal { pos, .. } => (*pos, *pos),
            Cursor::Selection {
                range: Range { start, end },
                ..
            } => (*start, *end),
        };

        end.move_by(&self.rope, up, down, left, right);
        self.set_cursor(Cursor::from_range(&Range::from_points(start, end)));
    }

    pub fn select_by_range(&mut self, selection: &Range) {
        if selection.start == selection.end {
            self.cursor = Cursor::from_point(&selection.start);
        } else {
            self.cursor = Cursor::from_range(selection);
        }
    }

    pub fn select_until_beginning_of_line(&mut self) {
        let (mut start, mut end) = match self.cursor() {
            Cursor::Normal { pos, .. } => (*pos, *pos),
            Cursor::Selection {
                range: Range { start, end },
                ..
            } => (*start, *end),
        };

        if end == start && end.x == 0 && start.y > 0 {
            start.y -= 1;
            start.x = self.line_len(start.y);
        } else {
            end.x = 0;
        }

        self.set_cursor(Cursor::from_range(&Range::from_points(start, end)));
    }

    pub fn select_until_end_of_line(&mut self) {
        let (start, mut end) = match self.cursor() {
            Cursor::Normal { pos, .. } => (*pos, *pos),
            Cursor::Selection {
                range: Range { start, end },
                ..
            } => (*start, *end),
        };

        end.x = self.rope.line_len(end.y);
        if start == end && end.y < self.num_lines() {
            end.y += 1;
            end.x = 0;
        }

        self.set_cursor(Cursor::from_range(&Range::from_points(start, end)));
    }

    pub fn select_line(&mut self, y: usize) {
        self.set_cursor(Cursor::new(y, 0));
        self.select_until_end_of_line();
    }

    pub fn insert_char(&mut self, ch: char) {
        self.insert(&ch.to_string())
    }

    pub fn insert(&mut self, string: &str) {
        let string_count = string.chars().count();
        let (remove, insert_at, _end) = match &self.cursor {
            Cursor::Normal { pos, .. } => (None, pos, pos),
            Cursor::Selection { range, .. } => (Some(range), range.front(), range.back()),
        };

        if let Some(remove) = remove {
            self.rope.remove(&remove);
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

        let x_diff = string
            .chars()
            .rev()
            .position(|c| c == '\n')
            .map(|x| string_count - x - 1)
            .unwrap_or(string_count);

        let y = insert_at.y + y_diff;
        let x = if string.contains('\n') {
            x_diff
        } else {
            insert_at.x + x_diff
        };

        self.set_cursor(Cursor::new(y, x));
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

    pub fn insert_with_smart_indent(&mut self, ch: char) {
        match ch {
            '\n' => {
                self.insert_char('\n');
                self.inherit_indent(self.main_cursor_pos().y);
            }
            '}' => {
                if matches!(self.cursor, Cursor::Normal { pos, .. } if self.indent_size(pos.y) == self.line_len(pos.y))
                {
                    self.back_tab();
                }

                self.insert_char('}');
            }
            _ => {
                self.insert_char(ch);
            }
        }
    }

    fn inherit_indent(&mut self, y: usize) {
        let prev_indent_size = if y > 0 { self.indent_size(y - 1) } else { 0 };

        let increase_depth = self
            .line_substr(
                y.saturating_sub(1),
                self.line_len(y.saturating_sub(1)).saturating_sub(1),
            )
            .ends_with('{');

        let n = if increase_depth {
            prev_indent_size + self.config.indent_size
        } else {
            prev_indent_size
        };

        for x in 0..n {
            self.rope.insert_char(
                &Point::new(y, x),
                match self.config.indent_style {
                    IndentStyle::Space => ' ',
                    IndentStyle::Tab => '\t',
                },
            );
        }

        self.cursor = Cursor::new(y, n);
    }

    fn do_indent(&mut self, pos: &Point) {
        // Should we do auto-indent?
        let prev_indent_size = if pos.y > 0 {
            self.indent_size(pos.y - 1)
        } else {
            0
        };

        let new_x = if pos.x == 0 && prev_indent_size > 0 {
            self.inherit_indent(pos.y);
        } else {
            let new_x = match self.config.indent_style {
                IndentStyle::Space => {
                    // Insert spaces until the next indentation level.
                    let n = self.config.indent_size - (pos.x % self.config.indent_size);
                    for _ in 0..n {
                        self.rope.insert_char(pos, ' ');
                    }
                    pos.x + n
                }
                IndentStyle::Tab => {
                    self.rope.insert_char(pos, '\t');
                    pos.x + 1
                }
            };
            self.cursor = Cursor::new(pos.y, new_x);
        };
    }

    pub fn do_deindent(&mut self, pos: &Point) {
        let n = min(
            self.indent_size(pos.y),
            if pos.x % self.config.indent_size == 0 {
                self.config.indent_size
            } else {
                pos.x % self.config.indent_size
            },
        );
        if n > 0 {
            let start = Point::new(pos.y, 0);
            let end = Point::new(pos.y, n);
            self.rope.remove(&Range::from_points(start, end));
            self.cursor = Cursor::new(pos.y, pos.x.saturating_sub(n));
        } else {
            self.cursor = Cursor::new(pos.y, pos.x);
        }
    }

    pub fn tab(&mut self) {
        match self.cursor.clone() {
            Cursor::Normal { pos, .. } => {
                self.do_indent(&pos);
            }
            Cursor::Selection { range, .. } => {
                let end_y = if range.back().x == 0 {
                    max(range.front().y, range.back().y.saturating_sub(1))
                } else {
                    range.back().y
                };
                for y in (range.front().y..=end_y).rev() {
                    self.do_indent(&Point::new(y, self.indent_size(y)));
                }

                self.cursor = Cursor::from_range(&Range::new(
                    range.front().y,
                    0,
                    end_y,
                    self.line_len(end_y),
                ));
            }
        }
    }

    pub fn back_tab(&mut self) {
        match self.cursor.clone() {
            Cursor::Normal { pos, .. } => {
                self.do_deindent(&pos);
            }
            Cursor::Selection { range, .. } => {
                let end_y = if range.back().x == 0 {
                    max(range.front().y, range.back().y.saturating_sub(1))
                } else {
                    range.back().y
                };
                for y in (range.front().y..=end_y).rev() {
                    self.do_deindent(&Point::new(y, self.indent_size(y)));
                }
                self.cursor = Cursor::from_range(&Range::new(
                    range.front().y,
                    0,
                    end_y,
                    self.line_len(end_y),
                ));
            }
        }
    }

    pub fn backspace(&mut self) {
        // Move the cursor at the end of previous line if it is at EOF.
        if let Cursor::Normal { pos, .. } = &self.cursor {
            if pos.y > 0 && pos.y == self.num_lines() {
                self.cursor = Cursor::new(pos.y - 1, self.line_len(pos.y - 1));
            }
        }

        // Decrease the indentation level if the cursor is in an indent.
        if let Cursor::Normal { pos, .. } = &self.cursor {
            if pos.x > 0 && pos.x <= self.indent_size(pos.y) {
                self.back_tab();
                return;
            }
        }

        // Determine the range to be deleted.
        let range = match self.cursor() {
            Cursor::Normal { pos, .. } => {
                let start = if pos.y == 0 && pos.x == 0 {
                    return;
                } else if pos.x == 0 {
                    Point::new(pos.y - 1, self.rope.line_len(pos.y - 1))
                } else {
                    Point::new(pos.y, pos.x - 1)
                };

                Range::from_points(start, *pos)
            }
            Cursor::Selection { range, .. } => range.clone(),
        };

        self.rope.remove(&range);
        self.cursor = Cursor::from_point(range.front());
    }

    pub fn delete(&mut self) {
        // Determine the range to be deleted.
        let range = match self.cursor() {
            Cursor::Normal { pos, .. } => {
                let max_y = self.rope.num_lines();
                let max_x = self.rope.line_len(pos.y);
                let end = if pos.y == max_y && pos.x == max_x {
                    // At EOF.
                    return;
                } else if pos.x == max_x {
                    Point::new(pos.y + 1, 0)
                } else {
                    Point::new(pos.y, pos.x + 1)
                };

                Range::from_points(*pos, end)
            }
            Cursor::Selection { range, .. } => range.clone(),
        };

        self.rope.remove(&range);
        self.cursor = Cursor::from_point(range.front());
    }

    pub fn truncate(&mut self) {
        self.select_until_end_of_line();
        self.delete();
    }

    pub fn truncate_reverse(&mut self) {
        self.select_until_beginning_of_line();
        self.delete();
    }

    pub fn toggle_comment_out(&mut self) {
        let prefix = match self.language.comment_out {
            Some(prefix) => prefix,
            None => return,
        };

        let (old_x, y_start, y_end) = match self.cursor() {
            Cursor::Normal { pos, .. } => (pos.x, pos.y, pos.y),
            Cursor::Selection { range, .. } => {
                let end_y = if range.back().x == 0 {
                    max(range.front().y, range.back().y.saturating_sub(1))
                } else {
                    range.back().y
                };

                (0, range.front().y, end_y)
            }
        };

        let prefix_len = prefix.chars().count();
        let mut new_x = 0;
        for y in y_start..=y_end {
            let indent_size = self.indent_size(y);
            if self.line_substr(y, indent_size).starts_with(prefix) {
                new_x = old_x.saturating_sub(prefix_len);
                self.rope
                    .remove(&Range::new(y, indent_size, y, indent_size + prefix_len));
            } else {
                new_x = old_x + prefix_len;
                self.rope.insert(&Point::new(y, indent_size), prefix);
            }
        }

        if y_start == y_end {
            self.cursor = Cursor::new(y_start, new_x);
        } else {
            self.cursor = Cursor::from_range(&Range::new(y_start, 0, y_end, self.line_len(y_end)));
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

        if let Some(rope) = self.undo_stack.pop() {
            self.redo_stack.push(self.rope.clone());
            self.rope = rope;
        }
    }

    pub fn redo(&mut self) {
        if let Some(buf) = self.redo_stack.pop() {
            self.undo_stack.push(self.rope.clone());
            self.rope = buf;
        }
    }

    pub fn cut_selection(&mut self) -> String {
        let text = self.copy_selection();
        self.backspace();
        text
    }

    pub fn copy_selection(&mut self) -> String {
        let mut text = String::new();
        if let Cursor::Selection { range, .. } = &self.cursor {
            for chunk in self.rope.sub_str(range).chunks() {
                text += chunk;
            }
        }

        text
    }

    pub fn paste(&mut self, text: &str) {
        self.insert(text);
    }

    pub fn current_word(&self) -> Option<String> {
        let pos = match &self.cursor {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection {
                range: Range { start, .. },
                ..
            } => start,
        };

        self.rope.word_at(pos).map(|(_, word)| word)
    }

    pub fn current_word_range(&self) -> Option<Range> {
        let pos = match &self.cursor {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection {
                range: Range { start, .. },
                ..
            } => start,
        };

        self.rope.word_at(pos).map(|(range, _)| range)
    }

    pub fn prev_word_range(&self) -> Option<Range> {
        let pos = match &self.cursor {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection {
                range: Range { start, .. },
                ..
            } => start,
        };

        self.rope.prev_word_at(pos)
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
        for ch in self.rope.chars() {
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

        matches
            .iter()
            .filter(|(index, _)| *index == needle_chars.len())
            .map(|(_, start)| {
                let x = last_newline_idx
                    .map(|i| needle.len() - i - 1)
                    .unwrap_or_else(|| start.x + needle.len());
                Range::new(start.y, start.x, start.y + y_len, x)
            })
            .collect::<Vec<Range>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::test::Bencher;

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
        b.move_cursor(0, 0, 1, 0); // Move left
        b.delete();
        assert_eq!(b.text(), "Hello World");
        b.delete();
        assert_eq!(b.text(), "Hello World");
    }

    #[test]
    fn insert_at_eof() {
        let mut b = Buffer::new();
        b.insert("abc");
        b.move_cursor(0, 1, 0, 0);
        assert_eq!(b.cursor(), &Cursor::new(1, 0));
        b.insert_char('x');
        assert_eq!(b.text(), "abc\nx");
        assert_eq!(b.cursor(), &Cursor::new(1, 1));
        b.insert_char('y');
        assert_eq!(b.text(), "abc\nxy");
        assert_eq!(b.cursor(), &Cursor::new(1, 2));
    }

    #[test]
    fn backspace_at_eof() {
        let mut b = Buffer::new();
        b.insert("abc");
        b.set_cursor(Cursor::new(1, 0));
        b.backspace();
        assert_eq!(b.text(), "ab");
        assert_eq!(b.cursor(), &Cursor::new(0, 2));

        let mut b = Buffer::new();
        b.insert("abc\n");
        b.set_cursor(Cursor::new(1, 0));
        b.backspace();
        assert_eq!(b.text(), "abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 3));

        let mut b = Buffer::new();
        b.backspace();
        assert_eq!(b.text(), "");
        assert_eq!(b.cursor(), &Cursor::new(0, 0));
    }

    #[test]
    fn move_cursor() {
        let mut b = Buffer::new();
        b.move_cursor(1, 0, 0, 0); // Do nothing
        b.insert("A\nDEF\n12345");
        assert_eq!(b.cursor(), &Cursor::new(2, 5));
        b.move_cursor(0, 0, 1, 0); // Move right
        assert_eq!(b.cursor(), &Cursor::new(2, 4));
        b.move_cursor(1, 0, 0, 0);
        assert_eq!(b.cursor(), &Cursor::new(1, 3));
        b.move_cursor(0, 3, 0, 0);
        assert_eq!(b.cursor(), &Cursor::new(3, 0));
        b.move_cursor(0, 0, 1, 0); // Move left
        assert_eq!(b.cursor(), &Cursor::new(2, 5));
        b.move_cursor(0, 0, 0, 1); // Move right
        assert_eq!(b.cursor(), &Cursor::new(3, 0));
    }

    #[test]
    fn multibyte_characters() {
        let mut b = Buffer::new();
        b.insert("Hello 世界!");
        b.set_cursor(Cursor::new(0, 7));
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
        b.set_cursor(Cursor::new(0, 0));
        b.insert_char('a');
        b.insert_char('あ');
        b.insert_char('!');
        assert_eq!(b.text(), "aあ!");
    }

    #[test]
    fn move_down_cursor_in_wrapped_line() {
        // aあ|b   =>   aあb   =>   aあb
        // い12         い1|2       い12
        //                         |
        // (both are wrapped)
        let mut b = Buffer::new();
        b.insert("aあbい12");
        b.set_cursor(Cursor::new(0, 2));
        b.move_cursor_with_line_wrap(4, 0, 1);
        assert_eq!(b.cursor(), &Cursor::new(0, 5));
        b.move_cursor_with_line_wrap(4, 0, 1);
        assert_eq!(b.cursor(), &Cursor::new(1, 0));

        // wxyz          wxyz
        // aあ|b   =>   aあb
        // い12         い1|2
        //
        // (both are wrapped)
        let mut b = Buffer::new();
        b.insert("wxyzaあbい12");
        b.set_cursor(Cursor::new(0, 6));
        b.move_cursor_with_line_wrap(4, 0, 1);
        assert_eq!(b.cursor(), &Cursor::new(0, 9));

        // aあbc|   =>   aあbc
        // い            い|
        //
        // (both are wrapped)
        let mut b = Buffer::new();
        b.insert("aあbcい");
        b.set_cursor(Cursor::new(0, 4));
        b.move_cursor_with_line_wrap(5, 0, 1);
        assert_eq!(b.cursor(), &Cursor::new(0, 5));

        // 1行|目   =>   1行目
        // 2行目         2行|目
        //
        //   (two lines)
        let mut b = Buffer::new();
        b.insert("1行目\n2行目");
        b.set_cursor(Cursor::new(0, 2));
        b.move_cursor_with_line_wrap(5, 0, 1);
        assert_eq!(b.cursor(), &Cursor::new(1, 2));

        // aあbc| => aあbc
        //          |
        let mut b = Buffer::new();
        b.insert("aあbc");
        b.set_cursor(Cursor::new(0, 4));
        b.move_cursor_with_line_wrap(5, 0, 1);
        assert_eq!(b.cursor(), &Cursor::new(1, 0));

        // | =>
        //      |
        let mut b = Buffer::new();
        b.set_cursor(Cursor::new(0, 0));
        b.move_cursor_with_line_wrap(5, 0, 1);
        assert_eq!(b.cursor(), &Cursor::new(1, 0));
        b.move_cursor_with_line_wrap(5, 0, 10);
        assert_eq!(b.cursor(), &Cursor::new(1, 0));
    }

    #[test]
    fn move_up_cursor_in_wrapped_line() {
        // aあb     =>   aあ|b
        // い1|2         い12
        //
        // (both are wrapped)
        let mut b = Buffer::new();
        b.insert("aあbい12");
        b.set_cursor(Cursor::new(0, 5));
        b.move_cursor_with_line_wrap(4, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(0, 2));

        // wxyz         wxy|z
        // aあ|b   =>   aあb
        // い12         い12
        //
        // (both are wrapped)
        let mut b = Buffer::new();
        b.insert("wxyzaあbい12");
        b.set_cursor(Cursor::new(0, 6));
        b.move_cursor_with_line_wrap(4, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(0, 3));

        // 1行目   =>   1行|目
        // 2行|目       2行目
        //
        //   (two lines)
        let mut b = Buffer::new();
        b.insert("1行目\n2行目");
        b.set_cursor(Cursor::new(1, 2));
        b.move_cursor_with_line_wrap(5, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(0, 2));

        // abcd     =>   abc|d
        // 123|4         1234
        //
        //   (two lines)
        let mut b = Buffer::new();
        b.insert("abcd\n1234");
        b.set_cursor(Cursor::new(1, 3));
        b.move_cursor_with_line_wrap(10, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(0, 3));

        // ab       =>   ab|
        // 12345|        12345
        //
        //   (two lines)
        let mut b = Buffer::new();
        b.insert("ab\n12345");
        b.set_cursor(Cursor::new(1, 5));
        b.move_cursor_with_line_wrap(10, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(0, 2));

        // aあbc| => aあbc|
        let mut b = Buffer::new();
        b.insert("aあbc");
        b.set_cursor(Cursor::new(0, 4));
        b.move_cursor_with_line_wrap(5, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(0, 4));

        // | => |
        let mut b = Buffer::new();
        b.set_cursor(Cursor::new(0, 0));
        b.move_cursor_with_line_wrap(5, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(0, 0));
    }

    #[test]
    fn logical_cursor_x_when_moving_down() {
        // abc|d   =>   abcd
        //
        // 1234         123|4
        let mut b = Buffer::new();
        b.insert("abcd\n\n1234");
        b.set_cursor(Cursor::new(0, 3));
        b.move_cursor_with_line_wrap(30, 0, 2);
        assert_eq!(b.cursor(), &Cursor::new(2, 3));

        // abc|d   =>   abcd    =>   abcd    =>   abcd
        // xy           xy|          x|y          xy
        // 1234         1234         1234         1|234
        let mut b = Buffer::new();
        b.insert("abcd\nxy\n1234");
        b.set_cursor(Cursor::new(0, 3));
        b.move_cursor_with_line_wrap(30, 0, 1);
        assert_eq!(b.cursor(), &Cursor::new(1, 2));
        b.move_cursor(0, 0, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(1, 1));
        b.move_cursor_with_line_wrap(30, 0, 1);
        assert_eq!(b.cursor(), &Cursor::new(2, 1));
    }

    #[test]
    fn logical_cursor_x_when_moving_up() {
        // abcd    =>   abc|d
        //
        // 123|4        1234
        let mut b = Buffer::new();
        b.insert("abcd\n\n1234");
        b.set_cursor(Cursor::new(2, 3));
        b.move_cursor_with_line_wrap(30, 2, 0);
        assert_eq!(b.cursor(), &Cursor::new(0, 3));

        // abcd    =>   abcd    =>   abcd    =>   a|bcd
        // xy           xy|          x|y          xy
        // 123|4        1234         1234         1234
        let mut b = Buffer::new();
        b.insert("abcd\nxy\n1234");
        b.set_cursor(Cursor::new(2, 3));
        b.move_cursor_with_line_wrap(30, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(1, 2));
        b.move_cursor(0, 0, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(1, 1));
        b.move_cursor_with_line_wrap(30, 1, 0);
        assert_eq!(b.cursor(), &Cursor::new(0, 1));
    }

    #[test]
    fn single_selection() {
        let mut b = Buffer::new();
        b.insert("abXYZcd");
        b.set_cursor(Cursor::new(0, 2));

        // ab|XYZ|cd
        b.select(0, 0, 0, 3);
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 2, 0, 5)));

        // a|b|XYZcd  =>  a|XYZcd
        b.select(0, 0, 4, 0);
        b.backspace();
        assert_eq!(b.text(), "aXYZcd");
        assert_eq!(b.cursor(), &Cursor::new(0, 1));

        // a|XYZ|cd  =>  a|cd
        b.select(0, 0, 0, 3);
        b.backspace();
        assert_eq!(b.text(), "acd");
        assert_eq!(b.cursor(), &Cursor::new(0, 1));

        // ab|  =>  ab|
        // c        |c
        let mut b = Buffer::new();
        b.insert("ab\nc");
        b.set_cursor(Cursor::new(0, 2));
        b.select(0, 0, 0, 1);
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 2, 1, 0)));
    }

    #[test]
    fn single_selection_including_newlines() {
        // xy|A     xy|z
        // BCD  =>
        // E|z
        let mut b = Buffer::new();
        b.insert("xyA\nBCD\nEz");
        b.set_cursor(Cursor::from_range(&Range::new(0, 2, 2, 1)));
        b.backspace();
        assert_eq!(b.text(), "xyz");
        assert_eq!(b.cursor(), &Cursor::new(0, 2));

        // ab|      abX|c
        // |c   =>
        //
        let mut b = Buffer::new();
        b.insert("ab\nc");
        b.set_cursor(Cursor::from_range(&Range::new(0, 2, 1, 0)));
        b.insert("X");
        assert_eq!(b.text(), "abXc");
        assert_eq!(b.cursor(), &Cursor::new(0, 3));
    }

    #[test]
    fn move_to_beginning_of_line() {
        // abc
        // de
        let mut b = Buffer::new();
        b.insert("abc\nde");
        b.set_cursor(Cursor::new(0, 0));
        b.move_to_beginning_of_line();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        b.set_cursor(Cursor::new(1, 1));
        b.move_to_beginning_of_line();
        assert_eq!(b.cursor(), &Cursor::new(1, 0));
    }

    #[test]
    fn move_to_end_of_line() {
        // abc
        // de
        let mut b = Buffer::new();
        b.insert("abc\nde");
        b.set_cursor(Cursor::new(0, 1));
        b.move_to_end_of_line();
        assert_eq!(b.cursor(), &Cursor::new(0, 3));
    }

    #[test]
    fn select_until_end_of_line() {
        let mut b = Buffer::new();
        b.insert("abc");
        b.set_cursor(Cursor::new(0, 1));
        b.select_until_end_of_line();
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 1, 0, 3)),);
    }

    #[test]
    fn set_text() {
        let mut b = Buffer::from_str("");
        b.set_text("abc");
        assert_eq!(b.text(), "abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        let mut b = Buffer::from_str("123\n456");
        b.set_text("x");
        assert_eq!(b.text(), "x");
        assert_eq!(b.cursor(), &Cursor::new(0, 1));
    }

    #[test]
    fn truncate() {
        // abc|XYZ  =>  abc|
        let mut b = Buffer::new();
        b.insert("abcXYZ");
        b.set_cursor(Cursor::new(0, 3));
        b.truncate();
        assert_eq!(b.text(), "abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 3));

        // abc|      abc|xyz
        // xyz  =>
        let mut b = Buffer::new();
        b.insert("abc\nxyz");
        b.set_cursor(Cursor::new(0, 3));
        b.truncate();
        assert_eq!(b.text(), "abcxyz");
        assert_eq!(b.cursor(), &Cursor::new(0, 3));
    }

    #[test]
    fn truncate_reverse() {
        // abc|XYZ  =>  abc|
        let mut b = Buffer::new();
        b.insert("abcXYZ");
        b.set_cursor(Cursor::new(0, 3));
        b.truncate_reverse();
        assert_eq!(b.text(), "XYZ");
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        // abc       abc|xyz
        // |xyz  =>
        let mut b = Buffer::new();
        b.insert("abc\nxyz");
        b.set_cursor(Cursor::new(1, 0));
        b.truncate_reverse();
        assert_eq!(b.text(), "abcxyz");
        assert_eq!(b.cursor(), &Cursor::new(0, 3));
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
        b.set_cursor(Cursor::new(0, 9));
        assert_eq!(b.current_word(), Some("world".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 6, 0, 11)));

        // hello |world from rust
        b.set_cursor(Cursor::new(0, 6));
        assert_eq!(b.current_word(), Some("world".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 6, 0, 11)));

        // hello world| from rust
        b.set_cursor(Cursor::new(0, 11));
        assert_eq!(b.current_word(), Some("world".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 6, 0, 11)));

        // a b| c
        let mut b = Buffer::from_str("a b c");
        b.set_cursor(Cursor::new(0, 3));
        assert_eq!(b.current_word(), Some("b".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 2, 0, 3)));

        // |a b c
        let mut b = Buffer::from_str("a b c");
        b.set_cursor(Cursor::new(0, 0));
        assert_eq!(b.current_word(), Some("a".to_owned()));
        assert_eq!(b.current_word_range(), Some(Range::new(0, 0, 0, 1)));

        // a | b
        let mut b = Buffer::from_str("a  b");
        b.set_cursor(Cursor::new(0, 2));
        assert_eq!(b.current_word(), None);
        assert_eq!(b.current_word_range(), None);

        // |
        let mut b = Buffer::from_str("");
        b.set_cursor(Cursor::new(0, 0));
        assert_eq!(b.current_word(), None);
        assert_eq!(b.current_word_range(), None);
    }

    #[test]
    fn move_to_prev_word() {
        // abc 123|  =>  abc |123
        let mut b = Buffer::from_str("abc 123");
        b.set_cursor(Cursor::new(0, 7));
        b.move_to_prev_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 4));

        // abc |123  =>  |abc 123
        b.move_to_prev_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));
        b.move_to_prev_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        // abc 123  xy|z  =>  abc 123  |xyz  =>  abc |123  xyz  => |abc 123  xyz
        let mut b = Buffer::from_str("abc 123  xyz");
        b.set_cursor(Cursor::new(0, 11));
        b.move_to_prev_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 9));
        b.move_to_prev_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 4));
        b.move_to_prev_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        // a  =>  a
        // |b     |b
        let mut b = Buffer::from_str("a\nb");
        b.set_cursor(Cursor::new(1, 0));
        b.move_to_prev_word();
        assert_eq!(b.cursor(), &Cursor::new(1, 0));

        // (empty)
        let mut b = Buffer::from_str("");
        b.set_cursor(Cursor::new(0, 0));
        b.move_to_prev_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));
    }

    #[test]
    fn move_to_next_word() {
        // |abc 123  =>  abc| 123
        let mut b = Buffer::from_str("abc 123");
        b.set_cursor(Cursor::new(0, 0));
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 3));

        // abc| 123  =>  abc 123|
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 7));
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 7));

        // a|bc 123  xyz  =>  abc| 123  xyz  =>  abc 123|  xyz  => abc 123  xyz|
        let mut b = Buffer::from_str("abc 123  xyz");
        b.set_cursor(Cursor::new(0, 1));
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 3));
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 7));
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 12));

        // |  =>  |
        //
        let mut b = Buffer::from_str("\n");
        b.set_cursor(Cursor::new(0, 0));
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        // (empty)
        let mut b = Buffer::from_str("");
        b.set_cursor(Cursor::new(0, 0));
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        // |a  =>  a|
        // b       b
        let mut b = Buffer::from_str("a\nb");
        b.set_cursor(Cursor::new(0, 0));
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 1));
        b.move_to_next_word();
        assert_eq!(b.cursor(), &Cursor::new(0, 1));
    }

    #[test]
    fn prev_word_range() {
        // abc|
        let mut b = Buffer::from_str("abc");
        b.set_cursor(Cursor::new(0, 3));
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 0, 0, 3)));

        // abc xyz|
        let mut b = Buffer::from_str("abc xyz");
        b.set_cursor(Cursor::new(0, 7));
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 4, 0, 7)));

        // abc|xyz
        let mut b = Buffer::from_str("abcxyz");
        b.set_cursor(Cursor::new(0, 3));
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 0, 0, 3)));

        // abc xyz;|
        let mut b = Buffer::from_str("abc xyz;");
        b.set_cursor(Cursor::new(0, 8));
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 7, 0, 8)));

        // abc !@#|
        let mut b = Buffer::from_str("abc !@#");
        b.set_cursor(Cursor::new(0, 7));
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 3, 0, 7)));

        // ____abc
        let mut b = Buffer::from_str("    abc");
        b.set_cursor(Cursor::new(0, 4));
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 0, 0, 4)));

        // (empty)
        let mut b = Buffer::from_str("");
        b.set_cursor(Cursor::new(0, 0));
        assert_eq!(b.prev_word_range(), None);

        // abc
        // |
        let mut b = Buffer::from_str("abc\n");
        b.set_cursor(Cursor::new(1, 0));
        assert_eq!(b.prev_word_range(), Some(Range::new(0, 3, 1, 0)));
    }

    #[test]
    fn move_between_blocks() {
        // abc|         abc           abc          abc
        //
        // xyz    =>    |xyz    =>    xyz    =>    xyz
        //
        // 123          123           |123         123
        //                                         |
        let mut b = Buffer::from_str("abc\n\nxyz\n\n123");
        b.set_cursor(Cursor::new(0, 3));
        b.move_to_next_block();
        assert_eq!(b.cursor(), &Cursor::new(2, 0));
        b.move_to_next_block();
        assert_eq!(b.cursor(), &Cursor::new(4, 0));
        b.move_to_next_block();
        assert_eq!(b.cursor(), &Cursor::new(5, 0));
        b.move_to_next_block();
        assert_eq!(b.cursor(), &Cursor::new(5, 0));

        // abc          abc           abc           |abc
        //
        // xyz    =>    xyz    =>     |xyz    =>    xyz
        //
        // 123          |123          123           123
        // |
        b.move_to_prev_block();
        assert_eq!(b.cursor(), &Cursor::new(4, 0));
        b.move_to_prev_block();
        assert_eq!(b.cursor(), &Cursor::new(2, 0));
        b.move_to_prev_block();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));
        b.move_to_prev_block();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        let mut b = Buffer::new();
        b.move_to_prev_block();
        assert_eq!(b.cursor(), &Cursor::new(0, 0));
        b.move_to_next_block();
        assert_eq!(b.cursor(), &Cursor::new(1, 0));
    }

    #[test]
    fn find() {
        // 012345678901234567890
        // hello rust from rust
        //       ^^^^      ^^^^
        let mut b = Buffer::from_str("hello rust from rust");
        assert_eq!(
            &b.find("rust"),
            &[Range::new(0, 6, 0, 10), Range::new(0, 16, 0, 20),]
        );

        let mut b = Buffer::from_str("hello rust from rust");
        assert_eq!(&b.find("rrrrr"), &[]);

        let mut b = Buffer::from_str("abXYZ\nXYZab");
        assert_eq!(
            &b.find("XYZ"),
            &[Range::new(0, 2, 0, 5), Range::new(1, 0, 1, 3),]
        );

        let mut b = Buffer::from_str("abXY\nZab");
        assert_eq!(&b.find("XY\nZ"), &[Range::new(0, 2, 1, 1),]);

        let mut b = Buffer::from_str("");
        assert_eq!(&b.find("rrrrr"), &[]);
    }

    #[test]
    fn indent_by_tab() {
        let mut b = Buffer::new();
        b.set_cursor(Cursor::new(0, 0));
        b.tab();
        assert_eq!(&b.text(), "    ");
        assert_eq!(b.cursor(), &Cursor::new(0, 4));
        b.tab();
        assert_eq!(&b.text(), "        ");
        assert_eq!(b.cursor(), &Cursor::new(0, 8));

        let mut b = Buffer::from_str("abc");
        b.set_cursor(Cursor::new(0, 0));
        b.tab();
        assert_eq!(&b.text(), "    abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 4));
        b.tab();
        assert_eq!(&b.text(), "        abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 8));

        let mut b = Buffer::from_str("  abc");
        b.set_cursor(Cursor::new(0, 2));
        b.tab();
        assert_eq!(&b.text(), "    abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 4));
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
        b.set_cursor(Cursor::new(1, 0));
        b.tab();
        assert_eq!(&b.text(), "        foo();\n        ");
        assert_eq!(b.cursor(), &Cursor::new(1, 8));
    }

    #[test]
    fn indent_by_enter() {
        // Inherit 8 spaces.
        let mut b = Buffer::from_str("        foo();");
        b.set_cursor(Cursor::new(0, 14));
        b.insert_with_smart_indent('\n');
        assert_eq!(&b.text(), "        foo();\n        ");
        assert_eq!(b.cursor(), &Cursor::new(1, 8));
    }

    #[test]
    fn smart_indent() {
        let mut b = Buffer::new();
        b.insert("    if (expr) ");
        b.insert_with_smart_indent('{');
        b.insert_with_smart_indent('\n');
        assert_eq!(&b.text(), "    if (expr) {\n        ");
        assert_eq!(b.cursor(), &Cursor::new(1, 8));

        b.insert_with_smart_indent('}');
        assert_eq!(&b.text(), "    if (expr) {\n    }");
        assert_eq!(b.cursor(), &Cursor::new(1, 5));
    }

    #[test]
    fn indent_with_selection() {
        let mut b = Buffer::from_str("xyz");
        b.set_cursor(Cursor::from_range(&Range::new(0, 1, 0, 3)));
        b.tab();
        assert_eq!(&b.text(), "    xyz");
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 0, 0, 7)));

        let mut b = Buffer::from_str("    a\n    b");
        b.set_cursor(Cursor::from_range(&Range::new(0, 0, 1, 1)));
        b.tab();
        assert_eq!(&b.text(), "        a\n        b");
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 0, 1, 9)));

        let mut b = Buffer::from_str("    a\n   b\n  c\n d\nxyz");
        b.set_cursor(Cursor::from_range(&Range::new(0, 0, 4, 0)));
        b.tab();
        assert_eq!(&b.text(), "        a\n    b\n    c\n    d\nxyz");
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 0, 3, 5)));
    }

    #[test]
    fn indent_with_selection_multiple_times() {
        let mut b = Buffer::from_str("a\nb\nc");
        b.set_cursor(Cursor::from_range(&Range::new(0, 0, 3, 0)));

        b.tab();
        assert_eq!(&b.text(), "    a\n    b\n    c");
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 0, 2, 5)));

        b.tab();
        assert_eq!(&b.text(), "        a\n        b\n        c");
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 0, 2, 9)));

        b.tab();
        assert_eq!(&b.text(), "            a\n            b\n            c");
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 0, 2, 13)));
    }

    #[test]
    fn deindent() {
        let mut b = Buffer::from_str("");
        b.set_cursor(Cursor::new(0, 0));
        b.back_tab();
        assert_eq!(&b.text(), "");
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        let mut b = Buffer::from_str("    ");
        b.set_cursor(Cursor::new(0, 0));
        b.back_tab();
        assert_eq!(&b.text(), "");
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        // len < config.indent_size
        let mut b = Buffer::from_str("  ");
        b.set_cursor(Cursor::new(0, 0));
        b.back_tab();
        assert_eq!(&b.text(), "");
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        let mut b = Buffer::from_str("     ");
        b.set_cursor(Cursor::new(0, 5));
        b.back_tab();
        assert_eq!(&b.text(), "    ");
        assert_eq!(b.cursor(), &Cursor::new(0, 4));

        let mut b = Buffer::from_str("        abc");
        b.set_cursor(Cursor::new(0, 8));
        b.back_tab();
        assert_eq!(&b.text(), "    abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 4));
        b.back_tab();
        assert_eq!(&b.text(), "abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 0));
    }

    #[test]
    fn comment_out() {
        let mut b = Buffer::from_str("abc");
        b.set_language(&crate::language::CXX);
        b.set_cursor(Cursor::new(0, 1));
        b.toggle_comment_out();
        assert_eq!(&b.text(), "// abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 4));

        b.set_text("");
        b.set_cursor(Cursor::new(0, 0));
        b.toggle_comment_out();
        assert_eq!(&b.text(), "// ");
        assert_eq!(b.cursor(), &Cursor::new(0, 3));

        b.set_text("    x\n    y\n    z");
        b.set_cursor(Cursor::from_range(&Range::new(0, 0, 3, 0)));
        b.toggle_comment_out();
        assert_eq!(&b.text(), "    // x\n    // y\n    // z");
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 0, 2, 8)));
    }

    #[test]
    fn decomment_out() {
        let mut b = Buffer::from_str("// abc");
        b.set_language(&crate::language::CXX);
        b.set_cursor(Cursor::new(0, 0));
        b.toggle_comment_out();
        assert_eq!(&b.text(), "abc");
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        let mut b = Buffer::from_str("// ");
        b.set_language(&crate::language::CXX);
        b.set_cursor(Cursor::new(0, 0));
        b.toggle_comment_out();
        assert_eq!(&b.text(), "");
        assert_eq!(b.cursor(), &Cursor::new(0, 0));

        b.set_text("// \n\n// z");
        b.set_cursor(Cursor::from_range(&Range::new(0, 0, 2, 4)));
        b.toggle_comment_out();
        assert_eq!(&b.text(), "\n// \nz");
        assert_eq!(b.cursor(), &Cursor::from_range(&Range::new(0, 0, 2, 1)));
    }

    #[test]
    fn copy_and_paste() {
        let mut b = Buffer::from_str("aXYZb");
        b.set_cursor(Cursor::from_range(&Range::new(0, 1, 0, 4)));
        assert_eq!(b.copy_selection(), "XYZ");
        assert_eq!(b.text(), "aXYZb");
        assert_eq!(b.cut_selection(), "XYZ");
        assert_eq!(b.text(), "ab");
        b.paste("123");
        assert_eq!(b.text(), "a123b");
    }

    #[test]
    fn line_substr() {
        let mut b = Buffer::from_str("abc\nxyz");
        assert_eq!(b.line_substr(0, 0), "abc");

        let mut b = Buffer::from_str("abc\nxyz");
        assert_eq!(b.line_substr(0, 1), "bc");

        let mut b = Buffer::from_str("");
        assert_eq!(b.line_substr(0, 0), "");
    }

    #[bench]
    fn bench_line_substr(b: &mut Bencher) {
        let mut buffer = Buffer::new();
        for _ in 0..10000 {
            buffer.insert("0123456789");
        }

        b.iter(|| buffer.line_substr(0, 3000));
    }
}
