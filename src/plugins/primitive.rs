use crate::editor::{Command, CommandDefinition, Editor};
use crate::plugin::{Plugin, Manifest};
use crate::frontend::Event;

pub struct PrimitivePlugin {
}

static MANIFEST: Manifest = Manifest {
    commands: &[
        CommandDefinition {
            id: "buffer.insert",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "buffer.save",
            title: "Save",
            hidden: false,
        },
        CommandDefinition {
            id: "buffer.backspace",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "buffer.delete",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "buffer.cursor_up",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "buffer.cursor_down",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "buffer.cursor_left",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "buffer.cursor_right",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "screen.vsplit",
            title: "screen: Split Screen Vertically",
            hidden: false,
        },
        CommandDefinition {
            id: "screen.panel_next",
            title: "screen: Move To Next Panel",
            hidden: false,
        },
        CommandDefinition {
            id: "screen.panel_prev",
            title: "screen: Move To Previous Panel",
            hidden: false,
        },
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
        let screen = editor.screen_mut();
        let active_view = screen.active_view_mut();
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
            Command("screen.vsplit")       => screen.split_vertically(),
            Command("screen.panel_prev")   => screen.move_panel(-1),
            Command("screen.panel_next")   => screen.move_panel(1),
            _ => {}
        }
    }

    fn manifest(&self) -> &'static Manifest {
        &MANIFEST
    }
}
