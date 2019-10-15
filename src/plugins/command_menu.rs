use crate::editor::{Command, CommandDefinition, Editor};
use crate::plugin::{Plugin, Manifest};
use crate::frontend::Event;
use crate::screen::Mode;

pub struct FinderPlugin {
}

static MANIFEST: Manifest = Manifest {
    commands: &[
        CommandDefinition {
            id: "command_menu.open",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "command_menu.insert",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "command_menu.backspace",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "command_menu.move_up",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "command_menu.move_down",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "command_menu.quit",
            title: "",
            hidden: true,
        },
    ]
};

impl FinderPlugin {
    pub fn new() -> FinderPlugin {
        FinderPlugin {
        }
    }
}

impl Plugin for FinderPlugin {
    fn command(&mut self, editor: &mut Editor, cmd: &Command, event: &Event) {
        let mut selected_cmd = None;
        {
            let screen = editor.screen_mut();
            let command_menu = screen.command_menu_mut();
            match *cmd {
                Command("command_menu.insert") => {
                    match event {
                        Event::Char('\n') => {
                            if let Some(selected) = command_menu.enter() {
                                // FIXME: Avoid temporary copy.
                                selected_cmd = Some(selected.to_owned());
                            }

                            editor.screen_mut().set_mode(Mode::Buffer);
                        }
                        Event::Char(ch) => {
                            command_menu.textbox_mut().insert(*ch);
                            command_menu.filter();
                        }
                        _ => {}
                    }
                }
                Command("command_menu.move_up") => {
                    command_menu.move_selection(-1);
                }
                Command("command_menu.move_down") => {
                    command_menu.move_selection(1);
                }
                Command("command_menu.backspace") => {
                    command_menu.textbox_mut().backspace();
                    command_menu.filter();
                }
                Command("command_menu.open") => {
                    editor.screen_mut().set_mode(Mode::Finder)
                }
                Command("command_menu.quit") => {
                    command_menu.clear();
                    editor.screen_mut().set_mode(Mode::Buffer)
                }
                _ => {}
            }

            // We need to drop borrows here to invoke a command.
        }

        if let Some(cmd_def) = selected_cmd {
            editor.fire_event(Event::Finder(cmd_def.id.to_owned()));
        }
    }

    fn manifest(&self) -> &'static Manifest {
        &MANIFEST
    }
}
