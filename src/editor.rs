use std::rc::Rc;
use std::cmp::min;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver, RecvTimeoutError};
use std::time::{Instant, Duration};
use crate::buffer::{Buffer, BufferId};
use std::io::Stdout;
use crate::command_box::{CommandBox, PreviewItem, File, RequestBody, Location};
use crate::completion::WordCompJob;
use crate::view::View;
use crate::worker::Worker;
use crate::highlight::Highlighter;
use crate::fuzzy::FuzzySet;
use crate::terminal::{Terminal, KeyCode, KeyModifiers, KeyEvent};
use crate::rope::{Cursor, Range, Point};

pub enum NotificationLevel {
    Report,
    Info,
    Error,
}

pub struct Notification {
    pub level: NotificationLevel,
    pub message: String,
    pub created_at: Instant,
}

impl PartialEq for Notification {
    fn eq(&self, other: &Notification) -> bool {
        self.message == other.message
    }
}

pub struct Popup {
    items: Vec<String>,
    selected: usize,
}

impl Popup {
    pub fn new(items: Vec<String>) -> Popup {
        Popup {
            items,
            selected: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_str(&self) -> &str {
        &self.items[self.selected]
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_next(&mut self) {
        self.selected = min(self.selected + 1, self.len().saturating_sub(1));
    }
}

pub enum Event {
    Key(KeyEvent),
    NoCompletion,
    Completion {
        id: BufferId,
        items: FuzzySet,
    },
    Resize {
        rows: usize,
        cols: usize,
    }
}

#[derive(Clone)]
pub struct EventQueue {
    tx: Sender<Event>,
}

impl EventQueue {
    pub fn new(tx: Sender<Event>) -> EventQueue {
        EventQueue {
            tx,
        }
    }

    pub fn enqueue(&self, ev: Event) {
        self.tx.send(ev).unwrap();
    }
}

pub enum EditorMode {
    Normal,
    CommandBox,
}

pub struct Editor {
    mode: EditorMode,
    terminal: Terminal,
    current: Rc<RefCell<View>>,
    views: Vec<Rc<RefCell<View>>>,
    event_queue: Receiver<Event>,
    exited: bool,
    notifications: RefCell<Vec<Notification>>,
    popup: Option<Popup>,
    popup_selected: usize,
    worker: Worker,
    command_box: CommandBox,
    command_box_input: Buffer,
    workspace_dir: PathBuf,
}

impl Editor {
    pub fn new() -> Editor {
        let (tx, rx) = channel();
        let scratch_buffer = Rc::new(RefCell::new(Buffer::new()));
        scratch_buffer.borrow_mut().set_name("*scratch*");
        let scratch_view = Rc::new(RefCell::new(View::new(scratch_buffer)));

        Editor {
            mode: EditorMode::Normal,
            terminal: Terminal::new(EventQueue::new(tx.clone())),
            current: scratch_view.clone(),
            views: vec![scratch_view],
            event_queue: rx,
            exited: false,
            notifications: RefCell::new(Vec::new()),
            popup: None,
            popup_selected: 0,
            worker: Worker::new(EventQueue::new(tx)),
            command_box: CommandBox::new(),
            command_box_input: Buffer::new(),
            workspace_dir: PathBuf::from("."),
        }
    }

    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    pub fn open_file(&mut self, path: &Path) {
        let abspath = match path.canonicalize() {
            Ok(abspath) => abspath,
            Err(err) => {
                self.error(format!("failed to resolve path: {} ({})", path.display(), err));
                return;
            }
        };

        for view in &self.views {
            if let Some(view_path) = view.borrow().buffer().borrow().file() {
                if abspath == *view_path {
                    // We have already opened the file. Switch to the view.
                    self.current = view.clone();
                    return;
                }
            }
        }

        let mut buffer = match Buffer::open_file(path) {
            Ok(buffer) => buffer,
            Err(err) => {
                self.error(format!("failed to open: {} ({})", path.display(), err));
                return;
            }
        };

        buffer.set_name(path.file_name().unwrap().to_str().unwrap());

        let buffer_rc = Rc::new(RefCell::new(buffer));
        let view = Rc::new(RefCell::new(View::new(buffer_rc)));
        self.views.push(view.clone());
        self.current = view;
    }

    pub fn run(&mut self) {
        self.draw();
        loop {
            if self.exited {
                return;
            }

            match self.event_queue.recv() {
                Ok(ev) => {
                    let started_at = Instant::now();
                    let snapshot = self.current
                        .borrow().buffer().borrow().snapshot();

                    self.handle_event(ev);
                    while let Ok(ev) = self.event_queue.try_recv() {
                        self.handle_event(ev);
                    }

                    let current = self.current
                        .borrow().buffer().borrow().snapshot();
                    if snapshot.buffer_id != current.buffer_id || snapshot.buf != current.buf {
                        self.on_modified();
                    }

                    self.draw();
                    info!("took {} us", started_at.elapsed().as_micros());
                }
                Err(err) => {
                   warn!("failed recv from the event queue: {:?}", err);
                }
            }
        }
    }

    fn draw(&mut self) {
        let mut view = self.current.borrow_mut();
        self.terminal.draw(
            &mut *view,
            &*self.notifications.borrow(),
            &self.popup,
            match self.mode {
                EditorMode::CommandBox => Some((&self.command_box, &self.command_box_input)),
                EditorMode::Normal => None,
            }
        );
    }

    fn on_modified(&mut self) {
        let view = self.current.borrow();
        let buffer = view.buffer().borrow();
        let snapshot = buffer.snapshot();
    }

    fn notify<T: Into<String>>(&self, level: NotificationLevel, message: T) {
        self.notifications.borrow_mut().push(Notification {
            level,
            created_at: Instant::now(),
            message: message.into(),
        });
    }

    fn report<T: Into<String>>(&self, message: T) {
        self.notify(NotificationLevel::Report, message);
    }

    fn info<T: Into<String>>(&self, message: T) {
        self.notify(NotificationLevel::Info, message);
    }

    fn error<T: Into<String>>(&self, message: T) {
        self.notify(NotificationLevel::Error, message);
    }

    fn handle_event(&mut self, ev: Event) {
        match ev {
            Event::Key(key) => {
                match self.mode {
                    EditorMode::Normal => {
                        self.handle_key_event(key);
                    }
                    EditorMode::CommandBox => {
                        self.handle_key_event_in_command_box(key);
                    }
                }
            }
            Event::Resize { rows, cols } => {
                self.terminal.resize(rows, cols);
            }
            Event::NoCompletion => {
                self.clear_popup();
            }
            Event::Completion { id, items } => {
                self.handle_completion_event(id, items);
            }
        }
    }

    fn is_popup_active(&self) -> bool {
        self.popup.is_some()
    }

    fn clear_popup(&mut self) {
        self.popup = None;
    }

    fn handle_completion_event(&mut self, id: BufferId, items: FuzzySet) {
        let view = self.current.borrow();
        let buffer = view.buffer().borrow();
        if buffer.id() != id {
            drop(buffer);
            drop(view);
            self.clear_popup();
            return;
        }

        self.popup_selected = 0;
        self.popup = {
            buffer.current_word()
                .map(|current_word| {
                    let mut filtered = Vec::new();
                    for item in items.search(&current_word, 5) {
                        if item != current_word {
                            filtered.push(item.to_owned());
                        }
                    }

                    if filtered.is_empty() {
                        None
                    } else {
                        Some(Popup::new(filtered))
                    }
                })
                .unwrap_or(None)
        };
    }

    fn open_command_box(&mut self) {
        self.command_box_input.clear();
        self.command_box_input.insert("//exit\\nend");
        self.mode = EditorMode::CommandBox;
        self.command_box.open();
    }

    fn close_command_box(&mut self) {
        self.mode = EditorMode::Normal;
    }

    fn execute_command(&mut self, preview: bool) {
        use crate::command_box::{Request, RequestBody, Response, ResponseBody, PreviewItem};
        use crate::search::{list_files, grep_dir};

        let input = self.command_box_input.text();
        let mut words = input.splitn(2, ' ');
        let pat = words.next().unwrap();
        let script = words.next().unwrap_or("").to_owned();

        let body;
        if pat.starts_with("//") {
            // Search all files.
            match grep_dir(self.workspace_dir(), &pat[2..]) {
                Ok(locations) => {
                    body = RequestBody::Locations {
                        locations,
                    };
                }
                Err(err) => {
                    self.error(format!("grep: {}", err));
                    return;
                }
            }
        } else if pat.starts_with("/") {
            return;
        } else if pat.starts_with(">") {
            // Filter file paths.
            body = RequestBody::Files {
                files: list_files(self.workspace_dir(), &pat[1..])
            };
        } else {
            self.notify(NotificationLevel::Error, "invalid prefix");
            return;
        }

        let global = false;
        let req = Request {
            script: script.clone(),
            selected: self.command_box.selected(),
            global,
            preview,
            body,
        };

        match self.command_box.execute(req) {
            Ok(()) => {}
            Err(err) => {
                error!("ruby script error: {}", err);
                self.error(format!("{}", err));
            }
        }

        let last_stderr = self.command_box.last_stderr();
        if !last_stderr.is_empty() {
            error!("stderr from ruby script: {}\n{}", script, last_stderr);
        }

        let resp = self.command_box.last_response().cloned();
        match resp {
            Some(Response { body, .. }) => match body {
                ResponseBody::Preview { .. } => {}
                ResponseBody::GoTo { file, position } => {
                    self.open_file(&file.path);
                    self.close_command_box();
                }
                _ => {
                    self.close_command_box();
                },
            }
            _ => {},
        }
    }

    fn handle_key_event_in_command_box(&mut self, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let mut modified = false;
        let mut close = false;
        match (key.code, key.modifiers) {
            (KeyCode::Enter, NONE) => {
                self.execute_command(false);
            }
            (KeyCode::Esc, NONE) => {
                self.close_command_box();
                return;
            }
            (KeyCode::Up, NONE) => {
                self.command_box.move_up();
            }
            (KeyCode::Down, NONE) => {
                self.command_box.move_down();
            }
            (KeyCode::Char('a'), CTRL) => {
                self.command_box_input.move_to_beginning_of_line();
            }
            (KeyCode::Char('e'), CTRL) => {
                self.command_box_input.move_to_end_of_line();
            }
            (KeyCode::Left, NONE) => {
                self.command_box_input.move_cursors(0, 0, 1, 0);
            }
            (KeyCode::Right, NONE) => {
                self.command_box_input.move_cursors(0, 0, 0, 1);
            }
            (KeyCode::Char('k'), CTRL) => {
                self.command_box_input.truncate();
                modified = true;
            }
            (KeyCode::Char(ch), NONE)
            | (KeyCode::Char(ch), SHIFT) => {
                self.command_box_input.insert_char(ch);
                modified = true;
            }
            (KeyCode::Backspace, NONE) => {
                self.command_box_input.backspace();
                modified = true;
            }
            (KeyCode::Delete, NONE)
            | (KeyCode::Char('d'), CTRL) => {
                self.command_box_input.delete();
                modified = true;
            }
            _ => {
                trace!("unhandled key event: {:?}", key);
            }
        }

        if modified {
            self.execute_command(true);
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let view = self.current.borrow_mut();
        let mut buffer = view.buffer().borrow_mut();
        let mut clear_popup = true;
        let mut update_completion = false;
        match (key.code, key.modifiers) {
            (KeyCode::Up, NONE) if self.is_popup_active() => {
                self.popup.as_mut().unwrap().select_prev();
                clear_popup = false;
            }
            (KeyCode::Down, NONE) if self.is_popup_active() => {
                self.popup.as_mut().unwrap().select_next();
                clear_popup = false;
            }
            (KeyCode::Esc, NONE) if self.is_popup_active() => {
                // Clear the popup.
            }
            (KeyCode::Enter, NONE) | (KeyCode::Tab, NONE) if self.is_popup_active() => {
                let selected = self.popup.as_mut().unwrap().selected_str();
                if let Some(current_word_range) = buffer.current_word_range() {
                    buffer.select_by_ranges(&[current_word_range]);
                    buffer.backspace();
                    buffer.insert(selected);
                }
            }
            (KeyCode::Char('q'), CTRL) => {
                self.exited = true;
            }
            (KeyCode::Char('l'), CTRL) => {
                // TODO: centering and force redraw
            }
            (KeyCode::Char('s'), CTRL) => {
                match buffer.save() {
                    Ok(_) => {
                        self.info(format!("saved ({} lines)", buffer.num_lines()));
                    }
                    Err(err) => {
                        self.error(format!("failed to save: {}", err));
                    }
                }
            }
            (KeyCode::Char('x'), CTRL) => {
                drop(buffer);
                drop(view);
                self.open_command_box();
                return;
            }
            (KeyCode::Char('k'), CTRL) => {
                buffer.truncate();
                self.report("truncated");
            }
            (KeyCode::Char('z'), CTRL) => {
                buffer.undo();
                self.report("undo");
            }
            (KeyCode::Char('r'), CTRL) => {
                buffer.redo();
                self.report("redo");
            }
            (KeyCode::Char('a'), CTRL) => {
                buffer.move_to_beginning_of_line();
            }
            (KeyCode::Char('e'), CTRL) => {
                buffer.move_to_end_of_line();
            }
            (KeyCode::Char('b'), ALT) => {
                buffer.move_to_prev_word();
            }
            (KeyCode::Char('f'), ALT) => {
                buffer.move_to_next_word();
            }
            (KeyCode::Char('w'), ALT) => {
                if let Some(current_word) = buffer.current_word() {
                    let matches = &buffer.find(&current_word);
                    buffer.select_by_ranges(matches);
                }
            }
            (KeyCode::Char('w'), CTRL) => {
                if let Some(current_word_range) = buffer.current_word_range() {
                    buffer.select_by_ranges(&[current_word_range]);
                    buffer.backspace();
                }
            }
            (KeyCode::Char(ch), NONE) | (KeyCode::Char(ch), SHIFT) => {
                buffer.insert_char(ch);
                update_completion = true;
                clear_popup = false;
            }
            (KeyCode::Tab, NONE) => {
                buffer.tab();
            }
            (KeyCode::BackTab, NONE) => {
                buffer.back_tab();
            }
            (KeyCode::Enter, NONE) => {
                buffer.insert_char('\n');
            }
            (KeyCode::Backspace, NONE) => {
                buffer.backspace();
                update_completion = true;
            }
            (KeyCode::Delete, NONE) | (KeyCode::Char('d'), CTRL) => {
                buffer.delete();
                update_completion = true;
            }
            (KeyCode::PageUp, NONE) => {
                buffer.move_cursors(30, 0, 0, 0);
            }
            (KeyCode::PageDown, NONE) => {
                buffer.move_cursors(0, 30, 0, 0);
            }
            (KeyCode::Up, NONE) => {
                buffer.move_cursors(1, 0, 0, 0);
            }
            (KeyCode::Down, NONE) => {
                buffer.move_cursors(0, 1, 0, 0);
            }
            (KeyCode::Left, NONE) => {
                buffer.move_cursors(0, 0, 1, 0);
            }
            (KeyCode::Right, NONE) => {
                buffer.move_cursors(0, 0, 0, 1);
            }
            (KeyCode::Up, SHIFT) => {
                buffer.select(1, 0, 0, 0);
            }
            (KeyCode::Down, SHIFT) => {
                buffer.select(0, 1, 0, 0);
            }
            (KeyCode::Left, SHIFT) => {
                buffer.select(0, 0, 1, 0);
            }
            (KeyCode::Right, SHIFT) => {
                buffer.select(0, 0, 0, 1);
            }
            _ => {
                trace!("unhandled key event: {:?}", key);
            }
        }

        drop(buffer);
        drop(view);

        if clear_popup {
            self.clear_popup();
        }

        if update_completion {
            // Run a completion.
            let view = self.current.borrow_mut();
            let mut buffer = view.buffer().borrow_mut();
            let snapshot = buffer.snapshot();
            self.worker.request(Box::new(WordCompJob::new(snapshot)));
        }
    }
}
