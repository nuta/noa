use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc::{channel, Sender, Receiver, RecvTimeoutError};
use std::time::{Instant, Duration};
use crate::buffer::{Buffer, BufferId};
use crate::view::View;
use crate::worker::Worker;
use crate::fuzzy::FuzzySet;
use crate::terminal::{Terminal, KeyCode, KeyModifiers, KeyEvent};

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

pub enum Event {
    Key(KeyEvent),
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
        }
    }

    pub fn run(&mut self) {
        self.draw();
        loop {
            if self.exited {
                return;
            }

            match self.event_queue.recv_timeout(Duration::from_millis(100)) {
               Ok(ev) => {
                    let mut contains_comp = false; // FIXME:
                    if let Event::Completion { .. } = &ev { contains_comp = true }
                    self.handle_event(ev);
                    while let Ok(ev) = self.event_queue.try_recv() {
                        if let Event::Completion { .. } = &ev { contains_comp = true }
                        self.handle_event(ev);
                    }

                    self.draw();
                    if !contains_comp {
                        self.run_jobs();
                    }
               }
               Err(RecvTimeoutError::Timeout) => {
               }
               Err(err) => {
                   warn!("failed recv from the event queue: {:?}", err);
               }
            }
        }
    }

    fn draw(&mut self) {
        self.terminal.draw(
            &mut *self.current.borrow_mut(),
            &*self.notifications.borrow(),
            &self.popup,
        );
    }

    fn run_jobs(&mut self) {
        use crate::completion::WordCompJob;
        let current_view = self.current.borrow();
        let current_buffer =current_view.buffer().borrow();

        self.worker.request(
            Box::new(WordCompJob::new(current_buffer.snapshot()))
        );
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
                trace!("key = {:?}", key);
                self.handle_key_event(key);
            }
            Event::Resize { rows, cols } => {
                self.terminal.resize(rows, cols);
            }
            Event::Completion { id, items } => {
                let view = self.current.borrow();
                let buffer = view.buffer().borrow();
                if buffer.id() == id {
                    if let Some(current_word) = buffer.current_word() {
                        let mut lines = Vec::new();
                        for item in items.search(&current_word, 5) {
                            lines.push(item.to_owned());
                        }

                        if !lines.is_empty() {
                            self.popup = Some(Popup { lines, index: Some(0) });
                        }
                    }
                }
            }
        }
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
