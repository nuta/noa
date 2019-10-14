use crate::editor::{Command, Editor};
use crate::plugin::{Plugin, Manifest};
use crate::frontend::Event;
use crate::screen::Mode;

pub struct CommandMenuPlugin {
}

static MANIFEST: Manifest = Manifest {
    commands: &[
        Command("command_menu.open"),
        Command("command_menu.insert"),
        Command("command_menu.backspace"),
        Command("command_menu.move_up"),
        Command("command_menu.move_down"),
        Command("command_menu.quit"),
    ]
};

impl CommandMenuPlugin {
    pub fn new() -> CommandMenuPlugin {
        CommandMenuPlugin {
        }
    }
}

impl Plugin for CommandMenuPlugin {
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
                    editor.screen_mut().set_mode(Mode::CommandMenu)
                }
                Command("command_menu.quit") => {
                    command_menu.clear();
                    editor.screen_mut().set_mode(Mode::Buffer)
                }
                _ => {}
            }

            // We need to drop borrows here to invoke a command.
        }

        if let Some(cmd_name) = selected_cmd {
            editor.invoke_command(&Command(&cmd_name), Event::CommandMenu);
        }
    }

    fn manifest(&self) -> &'static Manifest {
        &MANIFEST
    }
}
