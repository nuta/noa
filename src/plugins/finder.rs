use crate::editor::{Command, CommandDefinition, Editor};
use crate::plugin::{Plugin, Manifest};
use crate::frontend::Event;
use crate::screen::Mode;

pub struct FinderPlugin {
}

static MANIFEST: Manifest = Manifest {
    commands: &[
        CommandDefinition {
            id: "finder.open",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "finder.insert",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "finder.backspace",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "finder.move_up",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "finder.move_down",
            title: "",
            hidden: true,
        },
        CommandDefinition {
            id: "finder.quit",
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
            let finder = screen.finder_mut();
            match *cmd {
                Command("finder.insert") => {
                    match event {
                        Event::Char('\n') => {
                            if let Some(selected) = finder.enter() {
                                // FIXME: Avoid temporary copy.
                                selected_cmd = Some(selected.to_owned());
                            }

                            editor.screen_mut().set_mode(Mode::Buffer);
                        }
                        Event::Char(ch) => {
                            finder.textbox_mut().insert(*ch);
                            finder.filter();
                        }
                        _ => {}
                    }
                }
                Command("finder.move_up") => {
                    finder.move_selection(-1);
                }
                Command("finder.move_down") => {
                    finder.move_selection(1);
                }
                Command("finder.backspace") => {
                    finder.textbox_mut().backspace();
                    finder.filter();
                }
                Command("finder.open") => {
                    editor.screen_mut().set_mode(Mode::Finder)
                }
                Command("finder.quit") => {
                    finder.clear();
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
