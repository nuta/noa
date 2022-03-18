use anyhow::Result;

use crate::ui::compositor::Compositor;

use crate::{editor::Editor, ui::surface::UIContext};

use super::Action;

pub struct PageUp;

impl Action for PageUp {
    fn name(&self) -> &'static str {
        "page_up"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor) -> Result<()> {
        editor.documents.current_mut().movement().scroll_up();
        Ok(())
    }
}

pub struct PageDown;

impl Action for PageDown {
    fn name(&self) -> &'static str {
        "page_down"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor) -> Result<()> {
        editor.documents.current_mut().movement().scroll_down();
        Ok(())
    }
}

pub struct Centering;

impl Action for Centering {
    fn name(&self) -> &'static str {
        "centering"
    }

    fn run(&self, editor: &mut Editor, compositor: &mut Compositor) -> Result<()> {
        let pos = editor
            .documents
            .current()
            .buffer()
            .main_cursor()
            .moving_position();

        editor
            .documents
            .current_mut()
            .view_mut()
            .centering(pos, (compositor.screen_size().height - 2) / 2);
        Ok(())
    }
}
