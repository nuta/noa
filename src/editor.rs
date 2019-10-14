use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::file::File;
use crate::screen::{Screen, Mode};
use crate::screen::View;
use crate::plugin::Plugin;
use crate::frontend::{FrontEnd, Event};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct Command(pub &'static str);


#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct BindTo {
    mode: Mode,
    event: Event,
}

impl BindTo {
    pub const fn new(mode: Mode, event: Event) -> BindTo {
        BindTo { mode, event }
    }
}

static DEFAULT_BINDINGS: &'static [(BindTo, Command)] = &[
    (BindTo::new(Mode::Buffer, Event::Ctrl('q')), Command("editor.quit")),
    (BindTo::new(Mode::Buffer, Event::Ctrl('s')), Command("buffer.save")),
    (BindTo::new(Mode::Buffer, Event::Backspace), Command("buffer.backspace")),
    (BindTo::new(Mode::Buffer, Event::Delete), Command("buffer.delete")),
    (BindTo::new(Mode::Buffer, Event::Up), Command("buffer.cursor_up")),
    (BindTo::new(Mode::Buffer, Event::Down), Command("buffer.cursor_down")),
    (BindTo::new(Mode::Buffer, Event::Left), Command("buffer.cursor_left")),
    (BindTo::new(Mode::Buffer, Event::Right), Command("buffer.cursor_right")),
];

pub struct Editor<'u> {
    /// An FrontEnd instance.
    ui: Box<dyn FrontEnd + 'u>,
    /// screen.
    screen: Screen,
    /// The current view's index in `views`.
    current_view_index: usize,
    /// Opened files.
    files: HashMap<PathBuf, Rc<RefCell<File>>>,
    /// Plugins.
    plugins: Vec<Rc<RefCell<dyn Plugin>>>,
    /// Commands.
    commands: HashMap<Command, Rc<RefCell<dyn Plugin>>>,
    /// Key mappings.
    bindings: HashMap<BindTo, Command>,
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
            bindings.insert(*event, *cmd);
        }

        Editor {
            screen,
            current_view_index: 0,
            ui: Box::new(ui),
            files: HashMap::new(),
            plugins: Vec::new(),
            commands: HashMap::new(),
            bindings,
            quit: false,
        }
    }

    // The mainloop. It may return if the user exited the editor.
    pub fn run(&mut self) {
        self.ui.render(&self.screen);
        loop {
            let event = self.ui.read_event();
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
        self.screen.current_panel_mut().add_view(view);
        Ok(())
    }

    pub fn add_plugin<'a>(&'a mut self, plugin: impl Plugin + 'a + 'static) {
        let manifest = plugin.manifest();
        let plugin_rc = Rc::new(RefCell::new(plugin));

        for cmd in manifest.commands {
            self.commands.insert(*cmd, plugin_rc.clone());
        }
        self.plugins.push(plugin_rc);
    }

    pub fn add_binding(&mut self, bind_to: BindTo, cmd: Command) {
        self.bindings.insert(bind_to, cmd);
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn screen_mut(&mut self) -> &mut Screen {
        &mut self.screen
    }

    fn process_event(&mut self, mode: Mode, event: Event) {
        let cmd = if let Event::Char(_) = event {
            Command("buffer.insert")
        } else {
            match self.bindings.get(&BindTo::new(mode, event)) {
                Some(ev) => ev.clone(),
                None => {
                warn!("no keymapping for event: {:?}", event);
                    return;
                }
            }
        };

        trace!("command: {:?}", cmd);
        if cmd == Command("editor.quit") {
            self.quit = true;
            return;
        }

        let plugin = match self.commands.get(&cmd) {
            Some(plugin) => plugin.clone(),
            None => {
                 warn!("unhandled command: {:?}", cmd);
                 return;
            }
        };

        plugin.borrow_mut().command(self, &cmd, &event);
    }
}
