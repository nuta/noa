use anyhow::{Context, Result};
use noa_buffer::cursor::{Position, Range};
use noa_compositor::Compositor;

use crate::{
    clipboard::{ClipboardData, SystemClipboardData},
    editor::Editor,
    finder::open_finder,
};

use super::Action;

pub struct Save;

impl Action for Save {
    fn name(&self) -> &'static str {
        "save"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor
            .documents
            .current_mut()
            .save_to_file(Some((&mut editor.jobs, editor.proxy.clone())));
        Ok(())
    }
}

pub struct OpenFilder;

impl Action for OpenFilder {
    fn name(&self) -> &'static str {
        "open_finder"
    }

    fn run(&self, editor: &mut Editor, compositor: &mut Compositor<Editor>) -> Result<()> {
        open_finder(editor, compositor);
        Ok(())
    }
}

pub struct BackspaceWord;

impl Action for BackspaceWord {
    fn name(&self) -> &'static str {
        "backspace_word"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().backspace_word();
        Ok(())
    }
}

pub struct Truncate;

impl Action for Truncate {
    fn name(&self) -> &'static str {
        "truncate"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().truncate();
        Ok(())
    }
}

pub struct Delete;

impl Action for Delete {
    fn name(&self) -> &'static str {
        "delete"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().delete();
        Ok(())
    }
}

pub struct MoveToBeginningOfLine;

impl Action for MoveToBeginningOfLine {
    fn name(&self) -> &'static str {
        "move_to_beginning_of_line"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().move_to_beginning_of_line();
        Ok(())
    }
}

pub struct MoveToEndOfLine;

impl Action for MoveToEndOfLine {
    fn name(&self) -> &'static str {
        "move_to_end_of_line"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().move_to_end_of_line();
        Ok(())
    }
}

pub struct MoveToNextWord;

impl Action for MoveToNextWord {
    fn name(&self) -> &'static str {
        "move_to_next_word"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().move_to_next_word();
        Ok(())
    }
}

pub struct MoveToPrevWord;

impl Action for MoveToPrevWord {
    fn name(&self) -> &'static str {
        "move_to_prev_word"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().move_to_prev_word();
        Ok(())
    }
}

pub struct FindCurrentWord;

impl Action for FindCurrentWord {
    fn name(&self) -> &'static str {
        "find_current_word"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        let doc = editor.documents.current_mut();
        let buffer = doc.buffer_mut();
        buffer.clear_secondary_cursors();
        let c = buffer.main_cursor();
        let word_range = if c.is_selection() {
            Some(c.selection())
        } else {
            buffer.current_word(c.moving_position())
        };

        if let Some(word_range) = word_range {
            let text = buffer.substr(word_range);
            editor.find_query.set_text(&text);
        }
        Ok(())
    }
}

pub struct SelectAllCurrentWord;

impl Action for SelectAllCurrentWord {
    fn name(&self) -> &'static str {
        "select_all_current_word"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        let doc = editor.documents.current_mut();
        let buffer = doc.buffer_mut();

        buffer.clear_secondary_cursors();
        let c = buffer.main_cursor();
        let word_range = if c.is_selection() {
            Some(c.selection())
        } else {
            buffer.current_word(c.moving_position())
        };

        if let Some(word_range) = word_range {
            let text = buffer.substr(word_range);
            let selections: Vec<Range> = buffer.find_iter(&text, Position::new(0, 0)).collect();
            for selection in selections {
                buffer.add_cursor(selection);
            }
        }
        Ok(())
    }
}

pub struct SelectPrevWord;

impl Action for SelectPrevWord {
    fn name(&self) -> &'static str {
        "select_prev_word"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().select_prev_word();
        Ok(())
    }
}

pub struct SelectNextWord;

impl Action for SelectNextWord {
    fn name(&self) -> &'static str {
        "select_next_word"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().select_next_word();
        Ok(())
    }
}

pub struct MoveLineUp;

impl Action for MoveLineUp {
    fn name(&self) -> &'static str {
        "move_lines_up"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().move_lines_up();
        Ok(())
    }
}

pub struct MoveLinesDown;

impl Action for MoveLinesDown {
    fn name(&self) -> &'static str {
        "move_lines_down"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().move_lines_down();
        Ok(())
    }
}

pub struct AddCursorsUp;

impl Action for AddCursorsUp {
    fn name(&self) -> &'static str {
        "add_cursors_up"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.documents.current_mut().movement().add_cursors_up();
        Ok(())
    }
}

pub struct AddCursorsDown;

impl Action for AddCursorsDown {
    fn name(&self) -> &'static str {
        "add_cursors_down"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.documents.current_mut().movement().add_cursors_down();
        Ok(())
    }
}

pub struct DuplicateLinesUp;

impl Action for DuplicateLinesUp {
    fn name(&self) -> &'static str {
        "duplicate_lines_up"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().duplicate_lines_up();
        Ok(())
    }
}

pub struct DuplicateLinesDown;

impl Action for DuplicateLinesDown {
    fn name(&self) -> &'static str {
        "duplicate_lines_down"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().duplicate_lines_down();
        Ok(())
    }
}

pub struct SelectUntilBeginningOfLine;

impl Action for SelectUntilBeginningOfLine {
    fn name(&self) -> &'static str {
        "select_until_beginning_of_line"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor
            .documents
            .current_mut()
            .movement()
            .select_until_beginning_of_line();
        Ok(())
    }
}

pub struct SelectUntilEndOfLine;

impl Action for SelectUntilEndOfLine {
    fn name(&self) -> &'static str {
        "select_until_end_of_line"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor
            .documents
            .current_mut()
            .movement()
            .select_until_end_of_line();
        Ok(())
    }
}

pub struct Cut;

impl Action for Cut {
    fn name(&self) -> &'static str {
        "cut"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        let doc = editor.documents.current();
        editor
            .clipboard
            .copy_into_clipboard(ClipboardData::from_buffer(doc.buffer()))
            .with_context(|| "failed to copy to clipboard")?;

        Ok(())
    }
}

pub struct Copy;

impl Action for Copy {
    fn name(&self) -> &'static str {
        "copy"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        let doc = editor.documents.current_mut();
        let buffer = doc.buffer_mut();
        match editor
            .clipboard
            .copy_from_clipboard()
            .with_context(|| "failed to copy from clipboard")?
        {
            SystemClipboardData::Ours(ClipboardData { texts }) => {
                let strs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                buffer.insert_multiple(&strs);
            }
            SystemClipboardData::Others(string) => {
                buffer.insert(&string);
            }
        }

        Ok(())
    }
}

pub struct Paste;

impl Action for Paste {
    fn name(&self) -> &'static str {
        "paste"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().truncate();
        Ok(())
    }
}

pub struct Undo;

impl Action for Undo {
    fn name(&self) -> &'static str {
        "undo"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().undo();
        Ok(())
    }
}

pub struct Redo;

impl Action for Redo {
    fn name(&self) -> &'static str {
        "redo"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().redo();
        Ok(())
    }
}

pub struct CommentOut;

impl Action for CommentOut {
    fn name(&self) -> &'static str {
        "comment_out"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.current_buffer_mut().toggle_line_comment_out();
        Ok(())
    }
}
