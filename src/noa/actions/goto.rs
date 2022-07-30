use std::path::Path;

use anyhow::Result;
use noa_buffer::cursor::Position;
use noa_compositor::compositor::Compositor;

use crate::editor::Editor;

use super::Action;

pub struct GoToLine;

impl Action for GoToLine {
    fn name(&self) -> &'static str {
        "goto_line"
    }

    fn run(&self, _editor: &mut Editor, compositor: &mut Compositor<Editor>) -> Result<()> {
        // TODO:
        Ok(())
    }
}
