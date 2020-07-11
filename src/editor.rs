use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Instant, Duration};
use crate::buffer::Buffer;
use crate::view::View;
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

pub enum Event {
    Key(KeyEvent),
    Resize {
        rows: usize,
        cols: usize,
    }
}

pub struct Editor {
    terminal: Terminal,
    current: Rc<RefCell<View>>,
    views: Vec<Rc<RefCell<View>>>,
    event_queue: Receiver<Event>,
    exited: bool,
    notifications: RefCell<Vec<Notification>>,
}

impl Editor {
    pub fn new() -> Editor {
        let (tx, rx) = channel();
        let scratch_buffer = Rc::new(RefCell::new(Buffer::new()));
        scratch_buffer.borrow_mut().set_name("*scratch*");
        let scratch_view = Rc::new(RefCell::new(View::new(scratch_buffer)));
        Editor {
            terminal: Terminal::new(tx),
            current: scratch_view.clone(),
            views: vec![scratch_view],
            event_queue: rx,
            exited: false,
            notifications: RefCell::new(Vec::new()),
        }
    }

    pub fn run(&mut self) {
        loop {
            if self.exited {
                return;
            }

            self.terminal.draw(
                &*self.current.borrow(),
                &*self.notifications.borrow()
            );

            match self.event_queue.recv_timeout(Duration::from_millis(100)) {
               Ok(ev) => {
                    self.handle_event(ev);
                    while let Ok(ev) = self.event_queue.try_recv() {
                        self.handle_event(ev);
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
            }
        }
    }
}
