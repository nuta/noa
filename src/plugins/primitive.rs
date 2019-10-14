use crate::editor::{Command, Editor};
use crate::plugin::{Plugin, Manifest};
use crate::frontend::Event;

pub struct PrimitivePlugin {
}

static MANIFEST: Manifest = Manifest {
    commands: &[
        Command("buffer.insert"),
        Command("buffer.save"),
        Command("buffer.backspace"),
        Command("buffer.delete"),
        Command("buffer.cursor_up"),
        Command("buffer.cursor_down"),
        Command("buffer.cursor_left"),
        Command("buffer.cursor_right"),
    ],
};

impl PrimitivePlugin {
    pub fn new() -> PrimitivePlugin {
        PrimitivePlugin {
        }
    }
}

impl Plugin for PrimitivePlugin {
    fn command(&mut self, editor: &mut Editor, cmd: &Command, event: &Event) {
        let active_view = editor.layout_mut().active_view_mut();
        match *cmd {
            Command("buffer.insert") => {
                if let Event::Char(ch) = event {
                    active_view.insert(*ch);
                }
            }
            Command("buffer.save") => {
                // FIXME: Print a warning if an error occurrs.
                active_view.file().save().unwrap();
            }
            Command("buffer.backspace")    => active_view.backspace(),
            Command("buffer.delete")       => active_view.delete(),
            Command("buffer.cursor_up")    => active_view.move_cursor(-1, 0),
            Command("buffer.cursor_down")  => active_view.move_cursor(1, 0),
            Command("buffer.cursor_left")  => active_view.move_cursor(0, -1),
            Command("buffer.cursor_right") => active_view.move_cursor(0, 1),
            _ => {}
        }
    }

    fn manifest(&self) -> &'static Manifest {
        &MANIFEST
    }
}
