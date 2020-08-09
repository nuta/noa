use std::rc::Rc;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver, RecvTimeoutError};
use std::time::{Instant, Duration};
use crate::buffer::{Buffer, BufferId};
use crate::completion::WordCompJob;
use crate::view::View;
use crate::worker::Worker;
use crate::highlight::Highlighter;
use crate::fuzzy::FuzzySet;
use crate::terminal::{Terminal, KeyCode, KeyModifiers, KeyEvent};
use std::io::Stdout;
use crate::finder::FinderModal;
use crate::rope::Cursor;

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
    pub lines: Vec<String>,
    pub index: Option<usize>,
}

pub trait Modal {
    fn draw(&self, stdout: &mut Stdout, y: usize, height: usize, width: usize);
    fn move_up(&mut self);
    fn move_down(&mut self);
    fn input(&mut self, editor: &mut Editor, new_text: &str, cursor: usize);
    fn execute(&mut self, editor: &mut Editor);
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

pub struct Editor {
    terminal: Terminal,
    current: Rc<RefCell<View>>,
    views: Vec<Rc<RefCell<View>>>,
    event_queue: Receiver<Event>,
    exited: bool,
    notifications: RefCell<Vec<Notification>>,
    popup: Option<Popup>,
    worker: Worker,
    modal: Option<Box<dyn Modal>>,
    modal_input: Buffer,
    workspace_dir: PathBuf,
}

impl Editor {
    pub fn new() -> Editor {
        let (tx, rx) = channel();
        let scratch_buffer = Rc::new(RefCell::new(Buffer::new()));
        scratch_buffer.borrow_mut().set_name("*scratch*");
        let scratch_view = Rc::new(RefCell::new(View::new(scratch_buffer)));

        Editor {
            terminal: Terminal::new(EventQueue::new(tx.clone())),
            current: scratch_view.clone(),
            views: vec![scratch_view],
            event_queue: rx,
            exited: false,
            notifications: RefCell::new(Vec::new()),
            popup: None,
            worker: Worker::new(EventQueue::new(tx)),
            modal: None,
            modal_input: Buffer::new(),
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
            &self.modal,
        );
    }

    fn on_modified(&mut self) {
        let view = self.current.borrow();
        let buffer = view.buffer().borrow();
        let snapshot = buffer.snapshot();

        // Kick background jobs.
        self.worker.request(Box::new(WordCompJob::new(snapshot)));
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
                if self.modal.is_some() {
                    self.handle_key_event_in_modal(key);
                } else {
                    self.handle_key_event(key);
                }
            }
            Event::Resize { rows, cols } => {
                self.terminal.resize(rows, cols);
            }
            Event::NoCompletion => {
                self.clear_completion();
            }
            Event::Completion { id, items } => {
                self.handle_completion_event(id, items);
            }
        }
    }

    fn clear_completion(&mut self) {
        self.popup = None;
    }

    fn handle_completion_event(&mut self, id: BufferId, items: FuzzySet) {
        let view = self.current.borrow();
        let buffer = view.buffer().borrow();
        if buffer.id() != id {
            drop(buffer);
            drop(view);
            self.clear_completion();
            return;
        }

        self.popup = buffer.current_word()
            .and_then(|current_word| {
                let mut lines = Vec::new();
                for item in items.search(&current_word, 5) {
                    if item != current_word {
                        lines.push(item.to_owned());
                    }
                }

                if lines.is_empty() {
                    None
                } else {
                    Some(Popup { lines, index: Some(0) })
                }
            });
    }

    fn handle_key_event_in_modal(&mut self, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let mut input_modified = false;
        let mut close_modal = false;
        let mut modal = self.modal.take().unwrap();
        match (key.code, key.modifiers) {
            (KeyCode::Enter, NONE) => {
                modal.execute(self);
                close_modal = true;
            }
            (KeyCode::Char('g'), CTRL) => {
                close_modal = true;
            }
            (KeyCode::Char('k'), CTRL) => {
                self.modal_input.truncate();
                input_modified = true;
            }
            (KeyCode::Char('a'), CTRL) => {
                self.modal_input.move_to_beginning_of_line();
                input_modified = true;
            }
            (KeyCode::Char('e'), CTRL) => {
                self.modal_input.move_to_end_of_line();
                input_modified = true;
            }
            (KeyCode::Up, NONE) => {
                modal.move_up();
            }
            (KeyCode::Down, NONE) => {
                modal.move_down();
            }
            (KeyCode::Left, NONE) => {
                self.modal_input.move_cursors(0, 0, 1, 0);
                input_modified = true;
            }
            (KeyCode::Right, NONE) => {
                self.modal_input.move_cursors(0, 0, 0, 1);
                input_modified = true;
            }
            (KeyCode::Char(ch), NONE) | (KeyCode::Char(ch), SHIFT) => {
                self.modal_input.insert_char(ch);
                input_modified = true;
            }
            (KeyCode::Backspace, NONE) => {
                self.modal_input.backspace();
                input_modified = true;
            }
            (KeyCode::Delete, NONE) | (KeyCode::Char('d'), CTRL) => {
                self.modal_input.delete();
                input_modified = true;
            }
            _ => {
                trace!("unhandled key event: {:?}", key);
            }
        }

        if input_modified {
            let cursor = match self.modal_input.cursors()[0] {
                Cursor::Normal { pos } => pos.x,
                _ => unreachable!()
            };

            modal.input(self, &self.modal_input.text().trim_end(), cursor);
        }

        if close_modal {
            self.modal = None;
        } else {
            self.modal = Some(modal);
        }
    }

    fn open_modal(&mut self, modal: Box<dyn Modal>) {
        self.modal = Some(modal);
        self.modal_input.clear();
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let view = self.current.borrow_mut();
        let mut buffer = view.buffer().borrow_mut();
        match (key.code, key.modifiers) {
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
            (KeyCode::Char('f'), CTRL) => {
                drop(buffer);
                drop(view);
                self.open_modal(Box::new(FinderModal::new()));
                self.info("opened finder modal");
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
            (KeyCode::Char(ch), NONE) | (KeyCode::Char(ch), SHIFT) => {
                buffer.insert_char(ch);
            }
            (KeyCode::Enter, NONE) => {
                buffer.insert_char('\n');
            }
            (KeyCode::Backspace, NONE) => {
                buffer.backspace();
            }
            (KeyCode::Delete, NONE) | (KeyCode::Char('d'), CTRL) => {
                buffer.delete();
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
    }
}
