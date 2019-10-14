use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use crate::file::File;
use crate::fuzzy::FuzzySetElement;
use crate::screen::{Screen, Mode};
use crate::screen::View;
use crate::plugin::Plugin;
use crate::frontend::{FrontEnd, Event};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct Command<'a>(pub &'a str);

pub struct CommandDefinition {
    pub id: &'static str,
    pub title: &'static str,
    pub hidden: bool,
}

impl FuzzySetElement for &'static CommandDefinition {
    fn as_str(&self) -> &str {
        self.id
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct BindTo {
    mode: Mode,
    event: Event,
}

impl BindTo {
    pub const fn new(mode: Mode, event: Event) -> BindTo {
        BindTo { mode, event }
    }
}

macro_rules! binding {
    ($mode:expr, $event:expr, $command:expr) => {
        (BindTo::new($mode, $event), Command($command))
    };
}

static DEFAULT_BINDINGS: &'static [(BindTo, Command)] = &[
    binding!(Mode::Buffer, Event::Ctrl('q'), "editor.quit"),
    binding!(Mode::Buffer, Event::Ctrl('s'), "buffer.save"),
    binding!(Mode::Buffer, Event::Ctrl('x'), "command_menu.open"),
    binding!(Mode::Buffer, Event::AnyChar,   "buffer.insert"),
    binding!(Mode::Buffer, Event::Backspace, "buffer.backspace"),
    binding!(Mode::Buffer, Event::Delete,    "buffer.delete"),
    binding!(Mode::Buffer, Event::Up,        "buffer.cursor_up"),
    binding!(Mode::Buffer, Event::Down,      "buffer.cursor_down"),
    binding!(Mode::Buffer, Event::Left,      "buffer.cursor_left"),
    binding!(Mode::Buffer, Event::Right,     "buffer.cursor_right"),
    binding!(Mode::Buffer, Event::Ctrl('o'), "screen.panel_prev"),
    binding!(Mode::Buffer, Event::Ctrl('p'), "screen.panel_next"),

    binding!(Mode::CommandMenu, Event::AnyChar,   "command_menu.insert"),
    binding!(Mode::CommandMenu, Event::Backspace, "command_menu.backspace"),
    binding!(Mode::CommandMenu, Event::Up,        "command_menu.move_up"),
    binding!(Mode::CommandMenu, Event::Down,      "command_menu.move_down"),
    binding!(Mode::CommandMenu, Event::Esc,       "command_menu.quit"),
    binding!(Mode::CommandMenu, Event::Ctrl('x'), "command_menu.quit"),
];

pub struct Editor<'u> {
    /// An FrontEnd instance.
    ui: Box<dyn FrontEnd + 'u>,
    /// The event queue.
    event_queue: (mpsc::Sender<Event>, mpsc::Receiver<Event>),
    /// The screen.
    screen: Screen,
    /// The current view's index in `views`.
    current_view_index: usize,
    /// Opened files.
    files: HashMap<PathBuf, Rc<RefCell<File>>>,
    /// Plugins.
    plugins: Vec<Rc<RefCell<dyn Plugin>>>,
    /// Commands.
    commands: HashMap<Command<'u>, &'static CommandDefinition>,
    /// Command handlers.
    handlers: HashMap<Command<'u>, Rc<RefCell<dyn Plugin>>>,
    /// Key mappings.
    bindings: HashMap<BindTo, Command<'u>>,
    /// It's true if the editor is quitting.
    quit: bool,
}

impl<'u> Editor<'u> {
    pub fn new(ui: impl FrontEnd + 'u) -> Editor<'u> {
        // Create the scratch buffer. Note that the scratch buffer and view
        // can't be removed in order to make current_view_index always valid.
        let scratch_file = Rc::new(RefCell::new(File::pseudo_file("*scratch*")));
        let scratch_view = View::new(scratch_file);

        let screen_size = ui.get_screen_size();
        let screen = Screen::new(scratch_view, screen_size.height, screen_size.width);

        // Register default key bindings.
        let mut bindings = HashMap::new();
        for (event, cmd) in DEFAULT_BINDINGS {
            bindings.insert(event.clone(), *cmd);
        }

        Editor {
            screen,
            event_queue: mpsc::channel(),
            current_view_index: 0,
            ui: Box::new(ui),
            files: HashMap::new(),
            plugins: Vec::new(),
            commands: HashMap::new(),
            handlers: HashMap::new(),
            bindings,
            quit: false,
        }
    }

    // The mainloop. It may return if the user exited the editor.
    pub fn run(&mut self) {
        self.ui.render(&self.screen);
        self.ui.init(self.event_queue.0.clone());
        loop {
            let event = self.event_queue.1.recv().unwrap();
            let current_mode = self.screen().mode();
            self.process_event(current_mode, event);
            if self.quit {
                return;
            }

            self.ui.render(&self.screen);
        }
    }

    pub fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
        let name = path.to_str().unwrap();
        let file = Rc::new(RefCell::new(File::open_file(name, path)?));
        let view = View::new(file);
        self.screen.current_panel_mut().set_view(view);
        Ok(())
    }

    pub fn add_plugin<'a>(&'a mut self, plugin: impl Plugin + 'a + 'static) {
        let manifest = plugin.manifest();
        let plugin_rc = Rc::new(RefCell::new(plugin));

        let command_menu = self.screen.command_menu_mut();
        let menu_elements = command_menu.elements_mut();
        for cmd in manifest.commands {
            self.commands.insert(Command(cmd.id), cmd);
            self.handlers.insert(Command(cmd.id), plugin_rc.clone());
            if !cmd.hidden {
                menu_elements.insert(cmd);
            }
        }

        self.plugins.push(plugin_rc);
    }

    pub fn add_binding(&mut self, bind_to: BindTo, cmd: Command<'u>) {
        self.bindings.insert(bind_to, cmd);
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn screen_mut(&mut self) -> &mut Screen {
        &mut self.screen
    }

    pub fn fire_event(&mut self, event: Event) {
        (self.event_queue.0).send(event).unwrap();
    }

    fn process_event(&mut self, mode: Mode, event: Event) {
        let event_key = match event {
            Event::Char(_) => Event::AnyChar,
            _ => event.clone(),
        };

        let temp_cmd_name;
        let temp_cmd;
        let cmd = match event {
            Event::CommandMenu(ref cmd_name) => {
                temp_cmd_name = cmd_name.to_owned();
                temp_cmd = Command(&temp_cmd_name);
                temp_cmd
            }
            _ => {
                match self.bindings.get(&BindTo::new(mode, event_key)) {
                    Some(ev) => ev.clone(),
                    None => {
                        warn!("no keymapping for event: {:?}", event);
                        return;
                    }
                }
            }
        };

        trace!("command: {:?}", cmd);
        let plugin = match self.handlers.get(&cmd) {
            Some(plugin) => plugin.clone(),
            None => {
                 warn!("unhandled command: {:?}", cmd);
                 return;
            }
        };

        plugin.borrow_mut().command(self, &cmd, &event);
    }

    pub fn quit(&mut self) {
        self.quit = true;
    }
}
