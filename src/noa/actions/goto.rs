use anyhow::Result;

use noa_compositor::compositor::Compositor;

use crate::editor::Editor;

use super::Action;

pub struct GoToLine;

impl Action for GoToLine {
    fn name(&self) -> &'static str {
        "goto_line"
    }

    fn run(&self, _editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        // TODO:
        Ok(())
    }
}
