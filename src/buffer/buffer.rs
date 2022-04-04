use std::{
    ops::Deref,
    path::Path,
    process::{Command, Stdio},
};

use noa_editorconfig::EditorConfig;
use noa_languages::{get_language_by_name, tree_sitter, Language};

use crate::{
    cursor::{Cursor, CursorId, CursorSet, Position, Range},
    mut_raw_buffer::{Change, MutRawBuffer},
    raw_buffer::RawBuffer,
    syntax::{ParserError, Syntax},
};

#[derive(Clone, PartialEq, Debug)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

impl From<lsp_types::TextEdit> for TextEdit {
    fn from(edit: lsp_types::TextEdit) -> Self {
        Self {
            range: edit.range.into(),
            new_text: edit.new_text,
        }
    }
}

struct UndoState {
    buf: RawBuffer,
    cursors: CursorSet,
}

pub struct Buffer {
    lang: &'static Language,
    syntax: Option<Syntax>,
    pub(crate) buf: MutRawBuffer,
    pub(crate) cursors: CursorSet,
    pub(crate) config: EditorConfig,
    undo_stack: Vec<UndoState>,
    redo_stack: Vec<UndoState>,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            lang: get_language_by_name("plain").unwrap(),
            syntax: None,
            buf: MutRawBuffer::new(),
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
            buf: MutRawBuffer::from_text(text),
            ..Default::default()
        }
    }

    pub fn from_reader<T: std::io::Read>(reader: T) -> std::io::Result<Buffer> {
        Ok(Buffer {
            buf: MutRawBuffer::from_reader(reader)?,
            ..Default::default()
        })
    }

    pub fn set_raw_buffer(&mut self, raw_buffer: RawBuffer) {
        self.select_whole_buffer();
        self.delete();
        self.insert(&raw_buffer.text());
    }

    pub fn set_from_reader<T: std::io::Read>(&mut self, reader: T) -> std::io::Result<()> {
        self.set_raw_buffer(RawBuffer::from_reader(reader)?);
        Ok(())
    }

    pub fn editorconfig(&self) -> &EditorConfig {
        &self.config
    }

    pub fn set_editorconfig(&mut self, config: EditorConfig) {
        self.config = config;
    }

    pub fn syntax(&self) -> Option<&Syntax> {
        self.syntax.as_ref()
    }

    pub fn set_syntax_tree(&mut self, new_tree: tree_sitter::Tree) {
        if let Some(syntax) = self.syntax.as_mut() {
            syntax.set_tree(new_tree);
        }
    }

    pub fn highlight<F>(&self, range: Range, mut callback: F)
    where
        F: FnMut(Range, &str),
    {
        let buffer = self.raw_buffer().clone();
        if let Some(syntax) = self.syntax.as_ref() {
            syntax.query_highlight(&buffer, range, &mut callback);
        }
    }

    pub fn language(&self) -> &'static Language {
        self.lang
    }

    pub fn set_language(&mut self, lang: &'static Language) -> Result<(), ParserError> {
        let syntax = Syntax::new(lang)?;
        self.lang = lang;
        self.syntax = Some(syntax);
        Ok(())
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
        F: FnMut(&mut Cursor, &MutRawBuffer),
    {
        self.cursors.foreach(|c, _past_cursors| {
            if c.is_main_cursor() {
                f(c, &self.buf);
                *c.selection_mut() = self.buf.clamp_range(c.selection());
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

    pub fn foreach_cursors<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut MutRawBuffer, &mut Cursor, &mut [Cursor]),
    {
        self.cursors.foreach(|c, past_cursors| {
            f(&mut self.buf, c, past_cursors);
        });
    }

    pub fn edit_selection_current_word<F>(&mut self, mut f: F)
    where
        F: FnMut(&str) -> String,
    {
        self.foreach_cursors(|buf, c, past_cursors| {
            if c.selection().is_empty() {
                // Select the current word.
                if let Some(selection) = buf.current_word(c.moving_position()) {
                    c.select_pos(selection);
                }
            }

            if !c.selection().is_empty() {
                let text = buf.substr(c.selection());
                let new_text = f(&text);
                buf.edit_at_cursor(c, past_cursors, &new_text);
            }
        });
    }

    pub fn deselect_cursors(&mut self) {
        self.cursors.foreach(|c, _past_cursors| {
            c.move_to(c.moving_position().y, c.moving_position().x);
        });
    }

    pub fn save_to_file_without_formatting(&mut self, path: &Path) -> std::io::Result<()> {
        // Write into a temporary file and then (hopefully atomically) move it
        // to `path`.
        let mut f = tempfile::NamedTempFile::new()?;
        self.buf.write_to(&mut f)?;
        f.persist(path)?;

        Ok(())
    }

    pub fn save_to_file(&mut self, path: &Path) -> std::io::Result<()> {
        self.ensure_insert_final_newline();
        self.save_to_file_without_formatting(path)
    }

    pub fn save_to_file_with_sudo(&mut self, path: &Path) -> std::io::Result<()> {
        self.ensure_insert_final_newline();

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

    /// Inserts a newline if the buffer doesn't end with a newline.
    fn ensure_insert_final_newline(&mut self) {
        let last_y = self.num_lines() - 1;
        let last_x = self.line_len(last_y);
        if self.config.insert_final_newline && last_x > 0 {
            self.apply_text_edit(&TextEdit {
                range: Range::from_single_position(Position::new(last_y, last_x)),
                new_text: "\n".to_string(),
            })
        }
    }

    pub fn clear(&mut self) {
        self.buf = MutRawBuffer::new();
        self.cursors = CursorSet::new();
    }

    pub fn insert_char(&mut self, c: char) {
        self.insert(&c.to_string());
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

    fn do_apply_text_edit(&mut self, edit: &TextEdit) {
        let out_of_bounds = edit.range.start.y >= self.num_lines()
            || edit.range.end.y >= self.num_lines()
            || edit.range.start.x > self.line_len(edit.range.start.y)
            || edit.range.end.x > self.line_len(edit.range.end.y);
        if out_of_bounds {
            warn!("ignoring out-of-bounds text edit: {:?}", edit);
            return;
        }

        let id = self.cursors.add_cursor(edit.range);
        let mut applied = false;
        self.cursors.foreach(|c, past_cursors| {
            if c.selection() == edit.range {
                self.buf.edit_at_cursor(c, past_cursors, &edit.new_text);
                applied = true;
            }
        });

        self.cursors.remove_cursor(id);
        debug_assert!(applied);
    }

    pub fn apply_text_edit(&mut self, edit: &TextEdit) {
        // Turn selections into a normal cursors so that edit.range won't merge
        // them.
        self.cursors.deselect_cursors();

        self.do_apply_text_edit(edit);
    }

    pub fn apply_text_edits(&mut self, mut edits: Vec<TextEdit>) {
        // Turn selections into a normal cursors so that edit.range won't merge
        // them.
        self.cursors.deselect_cursors();

        // Apply edits from the bottom of the buffer.
        edits.sort_by_key(|edit| edit.range.front());
        for edit in edits.iter().rev() {
            self.do_apply_text_edit(edit);
        }
    }

    /// Returns true if the buffer was modified since the last undo save.
    pub fn save_undo(&mut self) -> bool {
        if let Some(last_undo) = self.undo_stack.last() {
            if last_undo.buf == *self.buf.raw_buffer() {
                // No changes.
                return false;
            }
        }

        self.redo_stack.clear();
        self.undo_stack.push(UndoState {
            buf: self.buf.raw_buffer().clone(),
            cursors: self.cursors.clone(),
        });

        true
    }

    pub fn undo(&mut self) {
        if let Some(state) = self.undo_stack.pop() {
            self.set_raw_buffer(state.buf.clone());
            self.cursors = state.cursors.clone();
            self.redo_stack.push(state);
        }
    }

    pub fn redo(&mut self) {
        if let Some(state) = self.redo_stack.pop() {
            self.set_raw_buffer(state.buf.clone());
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

    pub fn clear_undo_and_redo_stacks(&mut self) {
        self.cursors.clear_undo_and_redo_stacks();
    }

    pub fn clear_recorded_changes(&mut self) -> Vec<Change> {
        self.buf.clear_changes()
    }
}

impl Default for Buffer {
    fn default() -> Buffer {
        Buffer::new()
    }
}

impl Deref for Buffer {
    type Target = MutRawBuffer;

    fn deref(&self) -> &MutRawBuffer {
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
    fn complicated_emojis() {
        // Note: the emoji is 3-characters wide: U+1F469 U+200D U+1F52C.
        let mut b = Buffer::new();
        b.insert("a");
        b.insert("👩‍🔬");
        assert_eq!(b.cursors(), &[Cursor::new(0, 4)]);

        b.backspace();
        assert_eq!(b.text(), "a");
        assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);
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
