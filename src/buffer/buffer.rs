use std::{
    fs::OpenOptions,
    ops::Deref,
    path::Path,
    process::{Command, Stdio},
};

use noa_editorconfig::{EditorConfig, IndentStyle};

use crate::{
    cursor::{Cursor, CursorSet, Position},
    raw_buffer::RawBuffer,
};

fn compute_desired_indent_len(buf: &RawBuffer, config: &EditorConfig, y: usize) -> usize {
    let (prev_indent_len, char_at_cursor) = if y == 0 {
        (0, None)
    } else {
        let prev_indent_len = buf.line_indent_len(y - 1);
        let pos_at_newline = Position::new(y - 1, buf.line_len(y - 1));
        let char_at_cursor = buf.char_iter(pos_at_newline).prev();
        (prev_indent_len, char_at_cursor)
    };

    match char_at_cursor {
        Some('{') => prev_indent_len + config.indent_size,
        _ => prev_indent_len,
    }
}

struct UndoState {
    buf: RawBuffer,
    cursors: CursorSet,
}

pub struct Buffer {
    buf: RawBuffer,
    cursors: CursorSet,
    config: EditorConfig,
    undo_stack: Vec<UndoState>,
    redo_stack: Vec<UndoState>,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            buf: RawBuffer::new(),
            cursors: CursorSet::new(),
            config: EditorConfig::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn raw_buffer(&self) -> &RawBuffer {
        &self.buf
    }

    pub fn from_text(text: &str) -> Buffer {
        Buffer {
            buf: RawBuffer::from_text(text),
            ..Default::default()
        }
    }

    pub fn from_reader<T: std::io::Read>(reader: T) -> std::io::Result<Buffer> {
        Ok(Buffer {
            buf: RawBuffer::from_reader(reader)?,
            ..Default::default()
        })
    }

    pub fn line_len(&self, y: usize) -> usize {
        self.buf.line_len(y)
    }

    pub fn config(&self) -> &EditorConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: &EditorConfig) {
        self.config = *config;
    }

    pub fn cursors(&self) -> &[Cursor] {
        self.cursors.as_slice()
    }

    pub fn main_cursor(&self) -> &Cursor {
        &self.cursors()[0]
    }

    pub fn set_cursors(&mut self, new_cursors: &[Cursor]) {
        self.cursors.set_cursors(new_cursors);
    }

    pub fn update_main_cursor_with<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Cursor, &RawBuffer),
    {
        let mut c = self.main_cursor().clone();
        f(&mut c, &self.buf);
        self.cursors.set_cursors(&[c]);
    }

    pub fn move_to_end_of_line(&mut self) {
        self.cursors.foreach(|c, _past_cursors| {
            let y = c.moving_position().y;
            *c = Cursor::new(y, self.buf.line_len(y));
        });
    }

    pub fn move_to_beginning_of_line(&mut self) {
        self.cursors.foreach(|c, _past_cursors| {
            *c = Cursor::new(c.moving_position().y, 0);
        });
    }

    pub fn move_to_next_word(&mut self) {
        todo!()
    }

    pub fn move_to_prev_word(&mut self) {
        todo!()
    }

    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        self.buf.write_to(f)?;
        Ok(())
    }

    pub fn save_to_file_with_sudo(&self, path: &Path) -> std::io::Result<()> {
        let magic = "sudo is available without password";
        let check_sudo_output = Command::new("sudo")
            .args(&["echo", magic])
            .stdin(Stdio::null())
            .output()?
            .stdout;

        match std::str::from_utf8(&check_sudo_output) {
            Ok(output) => {
                if !output.contains(magic) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "sudo requires an interaction (password?)",
                    ));
                }
            }
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "non UTF-8 output from sudo",
                ))
            }
        }

        let mut use_sudo = Command::new("sudo")
            .arg("tee")
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = use_sudo.stdin.take().unwrap();
        self.buf.write_to(stdin)?;

        Ok(())
    }

    pub fn insert_char(&mut self, c: char) {
        self.insert(&c.to_string());
    }

    pub fn insert_newline_and_indent(&mut self) {
        // Insert a newline.
        self.cursors
            .foreach(|c, past_cursors| self.buf.edit_at_cursor(c, past_cursors, "\n"));

        // Add indentation.
        self.cursors.foreach(|c, past_cursors| {
            let indent_size = compute_desired_indent_len(&self.buf, &self.config, c.front().y);
            self.buf.edit_at_cursor(
                c,
                past_cursors,
                &match self.config.indent_style {
                    IndentStyle::Tab => "\t".repeat(indent_size),
                    IndentStyle::Space => " ".repeat(indent_size),
                },
            )
        });
    }

    pub fn indent(&mut self) {
        // How many indentation characters should we add for each cursors?
        let mut increase_lens = Vec::new();
        for c in &self.cursors {
            let pos = c.front();

            let desired_len = compute_desired_indent_len(&self.buf, &self.config, pos.y);
            let current_indent_len = self.buf.line_indent_len(pos.y);
            let n = if pos.x < desired_len && pos.x == current_indent_len {
                desired_len - pos.x
            } else {
                let mut x = pos.x + 1;
                while x % self.config.indent_size != 0 {
                    x += 1;
                }
                x - pos.x
            };

            increase_lens.push(n);
        }

        // Insert indentations.
        let mut increase_lens_iter = increase_lens.iter();
        self.cursors.foreach(|c, past_cursors| {
            let indent_size = *increase_lens_iter.next().unwrap();
            self.buf.edit_at_cursor(
                c,
                past_cursors,
                &match self.config.indent_style {
                    IndentStyle::Tab => "\t".repeat(indent_size),
                    IndentStyle::Space => " ".repeat(indent_size),
                },
            );
        });
    }

    pub fn deindent(&mut self) {
        self.cursors.foreach(|_c, _past_cursors| {
            // let n = min(
            //     self.buf
            //         .char(Position::new(y, 0))
            //         .take_while(|c| *c == ' ' || *c == '\t')
            //         .count(),
            //     self.config.indent_size,
            // );
            // self.buf.edit_cursor(Range::new(y, 0, y, n), "")
            todo!()
        });
    }

    pub fn insert(&mut self, s: &str) {
        self.cursors.foreach(|c, past_cursors| {
            self.buf.edit_at_cursor(c, past_cursors, s);
        });
    }

    /// A special insertion method for pasting different texts for each cursor.
    pub fn insert_multiple(&mut self, texts: &[&str]) {
        if texts.len() == self.cursors().len() {
            self.insert(&texts.join("\n"));
        }

        let mut texts_iter = texts.iter();
        self.cursors.foreach(|c, past_cursors| {
            self.buf
                .edit_at_cursor(c, past_cursors, texts_iter.next().unwrap());
        });
    }

    pub fn backspace(&mut self) {
        self.cursors.foreach(|c, past_cursors| {
            if c.selection().is_empty() {
                c.expand_left(&self.buf);
            }
            self.buf.edit_at_cursor(c, past_cursors, "");
        });
    }

    pub fn delete(&mut self) {
        self.cursors.foreach(|c, past_cursors| {
            if c.selection().is_empty() {
                c.expand_right(&self.buf);
            }
            self.buf.edit_at_cursor(c, past_cursors, "");
        });
    }

    pub fn save_undo(&mut self) {
        if let Some(last_undo) = self.undo_stack.last() {
            if last_undo.buf == self.buf {
                // No changes.
                return;
            }
        }

        self.redo_stack.clear();
        self.undo_stack.push(UndoState {
            buf: self.buf.clone(),
            cursors: self.cursors.clone(),
        });
    }

    pub fn undo(&mut self) {
        if let Some(state) = self.undo_stack.pop() {
            self.buf = state.buf.clone();
            self.cursors = state.cursors.clone();
            self.redo_stack.push(state);
        }
    }

    pub fn redo(&mut self) {
        if let Some(state) = self.redo_stack.pop() {
            self.buf = state.buf.clone();
            self.cursors = state.cursors.clone();
            self.redo_stack.push(state);
        }
    }
}

impl Default for Buffer {
    fn default() -> Buffer {
        Buffer::new()
    }
}

impl Deref for Buffer {
    type Target = RawBuffer;

    fn deref(&self) -> &RawBuffer {
        &self.buf
    }
}
