use std::{
    cmp::min,
    fs::OpenOptions,
    ops::Deref,
    path::Path,
    process::{Command, Stdio},
};

use noa_editorconfig::{EditorConfig, IndentStyle};
use noa_languages::{
    definitions::PLAIN,
    language::{Language, SyntaxSpan},
};

use crate::{
    cursor::{Cursor, CursorId, CursorSet, Position, Range},
    extras::indent::compute_desired_indent_len,
    raw_buffer::RawBuffer,
    syntax::Syntax,
};

struct UndoState {
    buf: RawBuffer,
    cursors: CursorSet,
}

pub struct Buffer {
    lang: &'static Language,
    syntax: Option<Syntax>,
    pub(crate) buf: RawBuffer,
    pub(crate) cursors: CursorSet,
    pub(crate) config: EditorConfig,
    undo_stack: Vec<UndoState>,
    redo_stack: Vec<UndoState>,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            lang: &PLAIN,
            syntax: None,
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

    pub fn syntax(&self) -> Option<&Syntax> {
        self.syntax.as_ref()
    }

    pub fn highlight<F>(&mut self, mut callback: F)
    where
        F: FnMut(Range, SyntaxSpan),
    {
        let buffer = self.raw_buffer().clone();
        if let Some(syntax) = self.syntax.as_mut() {
            syntax.highlight(&mut callback, &buffer);
        }
    }

    pub fn set_language(&mut self, lang: &'static Language) {
        self.lang = lang;
        self.syntax = Syntax::new(lang);
        if let Some(syntax) = self.syntax.as_mut() {
            syntax.update(&self.buf);
        }
    }

    pub fn cursors(&self) -> &[Cursor] {
        self.cursors.as_slice()
    }

    pub fn main_cursor(&self) -> &Cursor {
        self.cursors.main_cursor()
    }

    pub fn set_cursors_for_test(&mut self, new_cursors: &[Cursor]) {
        self.cursors.set_cursors_for_test(new_cursors);
    }

    pub fn update_cursors(&mut self, new_cursors: &[Cursor]) {
        debug_assert!(new_cursors.iter().any(|c| c.is_main_cursor()));
        self.cursors.update_cursors(new_cursors);
    }

    pub fn get_cursor_by_id(&mut self, id: CursorId) -> Option<&Cursor> {
        self.cursors.get_cursor_by_id(id)
    }

    pub fn add_cursor(&mut self, selection: Range) -> CursorId {
        assert_eq!(
            selection,
            self.clamp_range(selection),
            "tried to add a cursors with a out-of-buffer range",
        );

        self.cursors.add_cursor(selection)
    }

    fn clamp_range(&self, range: Range) -> Range {
        let mut r = range;
        r.start.y = min(r.start.y, self.num_lines().saturating_sub(1));
        r.end.y = min(r.start.y, self.num_lines().saturating_sub(1));
        r.start.x = min(r.start.x, self.line_len(r.start.y));
        r.end.x = min(r.end.x, self.line_len(r.end.y));
        r
    }

    pub fn clear_secondary_cursors(&mut self) {
        self.cursors.clear_secondary_cursors();
    }

    pub fn move_main_cursor_to_pos(&mut self, pos: Position) {
        self.set_main_cursor_with(|c, _| c.move_to_pos(pos));
    }

    pub fn select_main_cursor(
        &mut self,
        start_y: usize,
        start_x: usize,
        end_y: usize,
        end_x: usize,
    ) {
        self.select_main_cursor_range(Range::new(start_y, start_x, end_y, end_x));
    }

    pub fn select_main_cursor_range(&mut self, selection: Range) {
        self.set_main_cursor_with(|c, _| c.select_pos(selection));
    }

    pub fn set_main_cursor_with<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Cursor, &RawBuffer),
    {
        self.cursors.foreach(|c, _past_cursors| {
            if c.is_main_cursor() {
                f(c, &self.buf);
            }
        });
    }

    pub fn update_cursors_with<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Cursor, &Buffer),
    {
        let mut new_cursors = self.cursors().to_vec();
        for c in &mut new_cursors {
            f(c, self);
        }

        self.cursors.update_cursors(&new_cursors);
    }

    pub fn deselect_cursors(&mut self) {
        self.cursors.foreach(|c, _past_cursors| {
            c.move_to(c.moving_position().y, c.moving_position().x);
        });
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

    pub fn clear(&mut self) {
        self.buf = RawBuffer::new();
        self.cursors = CursorSet::new();
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

    pub fn delete_if_not_empty(&mut self) {
        self.cursors.foreach(|c, past_cursors| {
            if !c.selection().is_empty() {
                self.buf.edit_at_cursor(c, past_cursors, "");
            }
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

    pub fn undo_cursor_movements(&mut self) {
        self.cursors.undo_cursor_movements();
    }

    pub fn redo_cursor_movements(&mut self) {
        self.cursors.redo_cursor_movements();
    }

    // FIXME:
    pub fn post_update_hook(&mut self) {
        if let Some(syntax) = self.syntax.as_mut() {
            syntax.update(&self.buf);
        }

        self.cursors.clear_undo_and_redo_stacks();
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_line_len() {
        assert_eq!(Buffer::from_text("").line_len(0), 0);
        assert_eq!(Buffer::from_text("A").line_len(0), 1);
        assert_eq!(Buffer::from_text("A\n").line_len(0), 1);
        assert_eq!(Buffer::from_text("A\nBC").line_len(1), 2);
        assert_eq!(Buffer::from_text("A\nBC\n").line_len(1), 2);
    }

    #[test]
    fn insertion_and_backspace() {
        let mut b = Buffer::new();
        b.backspace();
        b.insert("Hello");
        b.insert(" World?");
        assert_eq!(b.text(), "Hello World?");
        b.backspace();
        assert_eq!(b.text(), "Hello World");
        b.insert_char('!');
        assert_eq!(b.text(), "Hello World!");
    }

    #[test]
    fn deletion() {
        // a|bc
        let mut b = Buffer::new();
        b.insert("abc");
        b.set_cursors_for_test(&[Cursor::new(0, 1)]);
        b.delete();
        assert_eq!(b.text(), "ac");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1),]);

        // a|
        // b
        let mut b = Buffer::new();
        b.insert("a\nb");
        b.set_cursors_for_test(&[Cursor::new(0, 1)]);
        b.delete();
        assert_eq!(b.text(), "ab");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1),]);
    }

    #[test]
    fn delete_selection() {
        // ab|XY        ab|cd
        // Z|cd|   =>
        let mut b = Buffer::new();
        b.insert("abXY\nZcd");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 2, 1, 1)]);
        b.delete();
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);
    }

    #[test]
    fn multibyte_characters() {
        let mut b = Buffer::new();
        b.insert("Hello 世界!");
        b.set_cursors_for_test(&[Cursor::new(0, 7)]);
        assert_eq!(b.len_chars(), 9);

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
    fn test_insertion_at_eof() {
        let mut b = Buffer::from_text("ABC");
        b.set_cursors_for_test(&[Cursor::new(0, 3)]);
        b.insert_char('\n');
        assert_eq!(b.text(), "ABC\n");
        assert_eq!(b.cursors(), &[Cursor::new(1, 0)]);

        let mut b = Buffer::from_text("");
        b.set_cursors_for_test(&[Cursor::new(0, 0)]);
        b.insert_char('A');
        assert_eq!(b.text(), "A");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);
    }

    #[test]
    fn test_multiple_cursors1() {
        // ABC
        // おは
        // XY
        let mut b = Buffer::from_text("ABC\nおは\nXY");
        b.set_cursors_for_test(&[Cursor::new(0, 1), Cursor::new(1, 1), Cursor::new(2, 1)]);
        b.insert("!");
        assert_eq!(b.text(), "A!BC\nお!は\nX!Y");
        b.backspace();
        assert_eq!(b.text(), "ABC\nおは\nXY");
    }

    #[test]
    fn test_multiple_cursors2() {
        // ABC
        // おは
        // XY
        let mut b = Buffer::from_text("ABC\nおは\nXY");
        b.set_cursors_for_test(&[
            Cursor::new_selection(0, 3, 1, 0),
            Cursor::new_selection(1, 2, 2, 0),
        ]);
        b.insert("!");
        assert_eq!(b.text(), "ABC!おは!XY");
        assert_eq!(b.cursors(), &[Cursor::new(0, 4), Cursor::new(0, 7)]);
    }

    #[test]
    fn test_multiple_cursors3() {
        // A|B| => |
        let mut b = Buffer::from_text("AB");
        b.set_cursors_for_test(&[Cursor::new(0, 1), Cursor::new(0, 2)]);
        b.backspace();
        assert_eq!(b.text(), "");
        assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);
    }

    #[test]
    fn backspace_on_multi_cursors() {
        // abc|      ab|
        // def|  =>  de|
        // xyz|      xy|
        let mut b = Buffer::new();
        b.insert("abc\ndef\nxyz");
        b.set_cursors_for_test(&[Cursor::new(0, 3), Cursor::new(1, 3), Cursor::new(2, 3)]);
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
        b.set_cursors_for_test(&[Cursor::new(0, 3), Cursor::new(1, 1), Cursor::new(2, 2)]);
        b.backspace();
        assert_eq!(b.text(), "ab\n\nxz");
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 2), Cursor::new(1, 0), Cursor::new(2, 1),]
        );

        // 1230|a|b|c|d|e|f => 123|f
        let mut b = Buffer::new();
        b.insert("1230abcdef");
        b.set_cursors_for_test(&[
            Cursor::new(0, 4),
            Cursor::new(0, 5),
            Cursor::new(0, 6),
            Cursor::new(0, 7),
            Cursor::new(0, 8),
            Cursor::new(0, 9),
        ]);
        b.backspace();
        assert_eq!(b.text(), "123f");
        assert_eq!(b.cursors(), &[Cursor::new(0, 3)]);

        // a|bc      |bc|12
        // |12   =>  wxy|
        // wxyz|
        let mut b = Buffer::new();
        b.insert("abc\n12\nwxyz");
        b.set_cursors_for_test(&[Cursor::new(0, 1), Cursor::new(1, 0), Cursor::new(2, 4)]);
        b.backspace();
        assert_eq!(b.text(), "bc12\nwxy");
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 0), Cursor::new(0, 2), Cursor::new(1, 3)]
        );

        // 0
        // |abc      0|abc|12|xyz
        // |12   =>
        // |xyz
        let mut b = Buffer::new();
        b.insert("0\nabc\n12\nxyz");
        b.set_cursors_for_test(&[Cursor::new(1, 0), Cursor::new(2, 0), Cursor::new(3, 0)]);
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
        b.set_cursors_for_test(&[
            Cursor::new(0, 2),
            Cursor::new(1, 0),
            Cursor::new(1, 1),
            Cursor::new(2, 0),
        ]);
        b.backspace();
        assert_eq!(b.text(), "adefg");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1), Cursor::new(0, 4)]);

        // ab|   =>  a|def|g
        // |c|def
        // |g
        let mut b = Buffer::new();
        b.insert("ab\ncdef\ng");
        b.set_cursors_for_test(&[
            Cursor::new(0, 2),
            Cursor::new(1, 0),
            Cursor::new(1, 1),
            Cursor::new(2, 0),
        ]);
        b.backspace();
        assert_eq!(b.text(), "adefg");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1), Cursor::new(0, 4)]);
    }

    #[test]
    fn delete_on_multi_cursors() {
        // a|Xbc|Yd
        let mut b = Buffer::new();
        b.insert("aXbcYd");
        b.set_cursors_for_test(&[Cursor::new(0, 1), Cursor::new(0, 4)]);
        b.delete();
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1), Cursor::new(0, 3)]);

        // a|b|
        let mut b = Buffer::new();
        b.insert("ab");
        b.set_cursors_for_test(&[Cursor::new(0, 1), Cursor::new(0, 2)]);
        b.delete();
        assert_eq!(b.text(), "a");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);

        // a|bc
        // d|ef
        // g|hi
        let mut b = Buffer::new();
        b.insert("abc\ndef\nghi");
        b.set_cursors_for_test(&[Cursor::new(0, 1), Cursor::new(1, 1), Cursor::new(2, 1)]);
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
        b.set_cursors_for_test(&[
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
        b.set_cursors_for_test(&[Cursor::new(0, 2), Cursor::new(1, 3)]);
        b.delete();
        assert_eq!(b.text(), "abcde");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2), Cursor::new(0, 5)]);

        // abc|
        // |d|ef
        // ghi|
        let mut b = Buffer::new();
        b.insert("abc\ndef\nghi");
        b.set_cursors_for_test(&[
            Cursor::new(0, 3),
            Cursor::new(1, 0),
            Cursor::new(1, 1),
            Cursor::new(2, 3),
        ]);
        b.delete();
        assert_eq!(b.text(), "abcf\nghi");
        assert_eq!(b.cursors(), &[Cursor::new(0, 3), Cursor::new(1, 3)]);

        // abc|     => abc|d|e|f
        // d|Xe|Yf
        let mut b = Buffer::new();
        b.insert("abc\ndXeYf");
        b.set_cursors_for_test(&[Cursor::new(0, 3), Cursor::new(1, 1), Cursor::new(1, 3)]);
        b.delete();
        assert_eq!(b.text(), "abcdef");
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 3), Cursor::new(0, 4), Cursor::new(0, 5),]
        );
    }

    #[test]
    fn multibyte_characters_regression1() {
        let mut b = Buffer::new();
        b.set_cursors_for_test(&[Cursor::new(0, 0)]);
        b.insert_char('a');
        b.insert_char('あ');
        b.insert_char('!');
        assert_eq!(b.text(), "aあ!");
    }

    #[test]
    fn single_selection_including_newlines() {
        let mut b = Buffer::from_text("A\nB");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 1, 1, 0)]);
        b.backspace();
        assert_eq!(b.text(), "AB");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);

        // xy|A     xy|z
        // BCD  =>
        // E|z
        let mut b = Buffer::new();
        b.insert("xyA\nBCD\nEz");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 2, 2, 1)]);
        b.backspace();
        assert_eq!(b.text(), "xyz");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);

        // ab|      abX|c
        // |c   =>
        //
        let mut b = Buffer::new();
        b.insert("ab\nc");
        b.set_cursors_for_test(&[Cursor::new_selection(0, 2, 1, 0)]);
        b.insert("X");
        assert_eq!(b.text(), "abXc");
        assert_eq!(b.cursors(), &[Cursor::new(0, 3)]);
    }

    #[test]
    fn multi_selections() {
        // ab|XYZ  =>  ab|
        // cd|XYZ  =>  cd|
        // ef|XYZ  =>  ef|
        let mut b = Buffer::new();
        b.insert("abXYZ\ncdXYZ\nefXYZ");
        b.set_cursors_for_test(&[
            Cursor::new_selection(0, 2, 0, 5),
            Cursor::new_selection(1, 2, 1, 5),
            Cursor::new_selection(2, 2, 2, 5),
        ]);
        b.delete();
        assert_eq!(b.text(), "ab\ncd\nef");
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 2), Cursor::new(1, 2), Cursor::new(2, 2),]
        );

        // ab|XY        ab|cd|ef
        // Z|cd|XY  =>
        // Z|ef
        let mut b = Buffer::new();
        b.insert("abXY\nZcdXY\nZef");
        b.set_cursors_for_test(&[
            Cursor::new_selection(0, 2, 1, 1),
            Cursor::new_selection(1, 3, 2, 1),
        ]);
        b.backspace();
        assert_eq!(b.text(), "abcdef");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2), Cursor::new(0, 4)]);

        // ab|XY        ab|cd|ef|g
        // Z|cd|XY  =>
        // Z|ef|XY
        // Z|g
        let mut b = Buffer::new();
        b.insert("abXY\nZcdXY\nZefXY\nZg");
        b.set_cursors_for_test(&[
            Cursor::new_selection(0, 2, 1, 1),
            Cursor::new_selection(1, 3, 2, 1),
            Cursor::new_selection(2, 3, 3, 1),
        ]);
        b.backspace();
        assert_eq!(b.text(), "abcdefg");
        assert_eq!(
            b.cursors(),
            &[Cursor::new(0, 2), Cursor::new(0, 4), Cursor::new(0, 6),]
        );
    }

    #[test]
    fn test_insert_newline_and_indent() {
        let mut b = Buffer::from_text("");
        b.set_cursors_for_test(&[Cursor::new(0, 0)]);
        b.insert_newline_and_indent();
        assert_eq!(b.config().indent_style, IndentStyle::Space);
        assert_eq!(b.config().indent_size, 4);
        assert_eq!(b.text(), "\n");
        assert_eq!(b.cursors(), &[Cursor::new(1, 0)]);

        let mut b = Buffer::from_text("        abXYZ");
        b.set_cursors_for_test(&[Cursor::new(0, 10)]);
        b.insert_newline_and_indent();
        assert_eq!(b.text(), "        ab\n        XYZ");
        assert_eq!(b.cursors(), &[Cursor::new(1, 8)]);
    }

    #[test]
    fn test_indent() {
        let mut b = Buffer::from_text("");
        b.set_cursors_for_test(&[Cursor::new(0, 0)]);
        b.indent();
        assert_eq!(b.config().indent_style, IndentStyle::Space);
        assert_eq!(b.config().indent_size, 4);
        assert_eq!(b.text(), "    ");

        //     abc
        let mut b = Buffer::from_text("    abc\n");
        b.set_cursors_for_test(&[Cursor::new(1, 0)]);
        b.indent();
        assert_eq!(b.text(), "    abc\n    ");

        // __
        let mut b = Buffer::from_text("  ");
        b.set_cursors_for_test(&[Cursor::new(0, 2)]);
        b.indent();
        assert_eq!(b.text(), "    ");

        // a
        let mut b = Buffer::from_text("a");
        b.set_cursors_for_test(&[Cursor::new(0, 1)]);
        b.indent();
        assert_eq!(b.text(), "a   ");

        // _____
        let mut b = Buffer::from_text("     ");
        b.set_cursors_for_test(&[Cursor::new(0, 5)]);
        b.indent();
        assert_eq!(b.text(), "        ");

        // if true {
        //     while true {
        let mut b = Buffer::from_text("if true {\n    while true {\n");
        b.set_cursors_for_test(&[Cursor::new(2, 0)]);
        b.indent();
        assert_eq!(b.text(), "if true {\n    while true {\n        ");

        // if true {
        //     while true {
        // __
        let mut b = Buffer::from_text("if true {\n    while true {\n  ");
        b.set_cursors_for_test(&[Cursor::new(2, 2)]);
        b.indent();
        assert_eq!(b.text(), "if true {\n    while true {\n        ");
    }

    #[test]
    fn truncate() {
        // ABCD
        let mut b = Buffer::new();
        b.insert("ABCD");
        b.set_cursors_for_test(&[Cursor::new(0, 2)]);

        b.truncate();
        assert_eq!(b.text(), "AB");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);
        b.truncate();
        assert_eq!(b.text(), "AB");
        assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);

        // ABCD
        //
        // XYZ
        let mut b = Buffer::new();
        b.insert("ABCD\n\nXYZ");
        b.set_cursors_for_test(&[Cursor::new(0, 4)]);

        b.truncate();
        assert_eq!(b.text(), "ABCD\nXYZ");
        assert_eq!(b.cursors(), &[Cursor::new(0, 4)]);
    }
}
