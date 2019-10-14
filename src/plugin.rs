use crate::frontend::Event;
use crate::editor::{Command, CommandDefinition, Editor};

pub struct Manifest {
    pub commands: &'static [CommandDefinition],
}

pub trait Plugin {
    fn command(&mut self, editor: &mut Editor, cmd: &Command, event: &Event);
    fn manifest(&self) -> &'static Manifest;
}
