use crate::ui::compositor::Compositor;
use anyhow::Result;

use crate::{editor::Editor, ui::surface::UIContext};

use super::Action;

pub struct ToUpperCase;

impl Action for ToUpperCase {
    fn name(&self) -> &'static str {
        "to_upper_case"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor) -> Result<()> {
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

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor) -> Result<()> {
        editor
            .documents
            .current_mut()
            .buffer_mut()
            .edit_selection_current_word(|text| text.to_ascii_lowercase());

        Ok(())
    }
}
