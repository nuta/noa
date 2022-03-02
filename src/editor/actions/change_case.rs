use anyhow::Result;
use noa_compositor::Compositor;

use crate::editor::Editor;

use super::Action;

pub struct ToUpperCase;

impl Action for ToUpperCase {
    fn name(&self) -> &'static str {
        "to_upper_case"
    }

    fn run(&mut self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor
            .documents
            .current_mut()
            .buffer_mut()
            .edit_selection_current_word(|text| text.to_ascii_uppercase());

        Ok(())
    }
}

pub struct ToLowerCase;

impl Action for ToLowerCase {
    fn name(&self) -> &'static str {
        "to_lower_case"
    }

    fn run(&mut self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor
            .documents
            .current_mut()
            .buffer_mut()
            .edit_selection_current_word(|text| text.to_ascii_lowercase());

        Ok(())
    }
}
