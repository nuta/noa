use std::any::Any;

use anyhow::Result;
use noa_compositor::Compositor;

use crate::editor::Editor;

mod change_case;
mod truncate;

pub trait Action: Any {
    fn name(&self) -> &'static str;
    fn run(&mut self, editor: &mut Editor, compositor: &mut Compositor<Editor>) -> Result<()>;
}

pub const ACTIONS: &[&dyn Action] = &[&change_case::ToUpperCase, &change_case::ToLowerCase];
