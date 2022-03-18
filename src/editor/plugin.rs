use crate::{editor::Editor, hook::HookManager, ui::compositor::Compositor};

pub struct PluginContext<'a> {
    pub editor: &'a mut Editor,
    pub compositor: &'a mut Compositor,
    pub hooks: &'a mut HookManager,
}

pub trait Plugin {
    fn name(&self) -> &'static str;
    fn init(&mut self, ctx: &mut PluginContext);
}
