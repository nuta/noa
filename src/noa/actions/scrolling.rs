use anyhow::Result;

use noa_compositor::compositor::Compositor;

use crate::editor::Editor;

use super::Action;

pub struct PageUp;

impl Action for PageUp {
    fn name(&self) -> &'static str {
        "page_up"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        // TODO:
        // editor.current_document_mut().scroll_up();
        Ok(())
    }
}

pub struct PageDown;

impl Action for PageDown {
    fn name(&self) -> &'static str {
        "page_down"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        // TODO:
        // editor.current_document_mut().scroll_down();
        Ok(())
    }
}
