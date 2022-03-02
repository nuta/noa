use anyhow::{Context, Result};
use noa_compositor::Compositor;

use crate::{
    clipboard::{ClipboardData, SystemClipboardData},
    editor::Editor,
    ui::finder_view::FinderView,
};

use super::Action;

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

pub struct Save;

impl Action for Save {
    fn name(&self) -> &'static str {
        "save"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.documents.current_mut().save_to_file()?;
        Ok(())
    }
}

pub struct OpenFilder;

impl Action for OpenFilder {
    fn name(&self) -> &'static str {
        "open_filder"
    }

    fn run(&self, _editor: &mut Editor, compositor: &mut Compositor<Editor>) -> Result<()> {
        compositor
            .get_mut_surface_by_name::<FinderView>("finder")
            .set_active(true);

        Ok(())
    }
}