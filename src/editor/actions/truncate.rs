use anyhow::Result;
use noa_compositor::Compositor;

use crate::editor::Editor;

use super::Action;

pub struct Truncate;

impl Action for Truncate {
    fn name(&self) -> &'static str {
        "truncate"
    }

    fn run(&self, editor: &mut Editor, compositor: &mut Compositor<Editor>) -> Result<()> {
        editor.documents.current_mut().buffer_mut().truncate();
        Ok(())
    }
}
