use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use crate::file::File;
use crate::fuzzy::FuzzySetElement;
use crate::highlight::HighlightManager;
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

    binding!(Mode::Finder, Event::AnyChar,   "command_menu.insert"),
    binding!(Mode::Finder, Event::Backspace, "command_menu.backspace"),
    binding!(Mode::Finder, Event::Up,        "command_menu.move_up"),
    binding!(Mode::Finder, Event::Down,      "command_menu.move_down"),
    binding!(Mode::Finder, Event::Esc,       "command_menu.quit"),
    binding!(Mode::Finder, Event::Ctrl('x'), "command_menu.quit"),
];

pub struct EventQueue {
    pub tx: mpsc::Sender<Event>,
    pub rx: mpsc::Receiver<Event>, 
}

pub struct Editor<'u> {
    /// An FrontEnd instance.
    ui: Box<dyn FrontEnd + 'u>,
    /// The event queue.
    event_queue: EventQueue,
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
    /// Global states (e.g. programming langauge definitions) of the syntax
    /// highlighter.
    highlight_manager: &'static HighlightManager,
    /// The current theme.
    theme_name: String,
}

impl<'u> Editor<'u> {
    pub fn new(ui: impl FrontEnd + 'u) -> Editor<'u> {
        // Create the scratch buffer. Note that the scratch buffer and view
        // can't be removed in order to make current_view_index always valid.
        let scratch_file = Rc::new(RefCell::new(File::pseudo_file("*scratch*")));
        let scratch_view = View::new(scratch_file);

        // Register default key bindings.
        let mut bindings = HashMap::new();
        for (event, cmd) in DEFAULT_BINDINGS {
            bindings.insert(event.clone(), *cmd);
        }

        let (tx, rx) = mpsc::channel();
        Editor {
            screen: Screen::new(scratch_view, ui.get_screen_size()),
            event_queue: EventQueue { tx, rx },
            current_view_index: 0,
            ui: Box::new(ui),
            files: HashMap::new(),
            plugins: Vec::new(),
            commands: HashMap::new(),
            handlers: HashMap::new(),
            bindings,
            quit: false,
            highlight_manager: HighlightManager::new(),
            theme_name: "Solarized (light)".to_owned(),
        }
    }

    // The mainloop. It may return if the user exited the editor.
    pub fn run(&mut self) {
        self.ui.render(&self.screen);
        self.ui.init(self.event_queue.tx.clone());
        loop {
            let event = self.event_queue.rx.recv().unwrap();
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
        let ext = path.extension()
            .map(|s| s.to_str().map(|s| s.to_owned()).unwrap_or(String::new()))
            .unwrap_or(String::new());

        let file = Rc::new(RefCell::new(File::open_file(name, path)?));
        let highlight =
            self.highlight_manager.create_highlight(&self.theme_name, &ext);
        file.borrow_mut().set_highlight(highlight);
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

    pub fn event_queue(&self) -> mpsc::Sender<Event> {
        self.event_queue.tx.clone()
    }

    pub fn fire_event(&mut self, event: Event) {
        self.event_queue.tx.send(event).unwrap();
    }

    fn process_event(&mut self, mode: Mode, event: Event) {
        let temp_cmd_name;
        let temp_cmd;
        let cmd = match event {
            Event::ScreenResized => {
                self.screen.resize(self.ui.get_screen_size());
                return;
            }
            Event::Finder(ref cmd_name) => {
                temp_cmd_name = cmd_name.to_owned();
                temp_cmd = Command(&temp_cmd_name);
                temp_cmd
            }
            _ => {
                let event_key = match event {
                    Event::Char(_) => Event::AnyChar,
                    _ => event.clone(),
                };

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
