use std::any::Any;

use anyhow::Result;
use noa_compositor::Compositor;

use crate::editor::Editor;

mod change_case;

pub trait Action: Any {
    fn run(&mut self, editor: &mut Editor, compositor: &mut Compositor<Editor>) -> Result<()>;
}
