use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::Duration;
use crate::buffer::Buffer;
use crate::terminal::{Terminal, KeyCode, KeyModifiers, KeyEvent};

pub enum Event {
    Key(KeyEvent),
    Resize {
        rows: u16,
        cols: u16,
    }
}

pub struct Editor {
    terminal: Terminal,
    current: Rc<RefCell<Buffer>>,
    buffers: Vec<Rc<RefCell<Buffer>>>,
    event_queue: Receiver<Event>,
    exited: bool,
}

impl Editor {
    pub fn new() -> Editor {
        let (tx, rx) = channel();
        let scratch = Rc::new(RefCell::new(Buffer::new()));
        Editor {
            terminal: Terminal::new(tx),
            current: scratch.clone(),
            buffers: vec![scratch],
            event_queue: rx,
            exited: false,
        }
    }

    pub fn run(&mut self) {
        loop {
            if self.exited {
                return;
            }

            match self.event_queue.recv_timeout(Duration::from_millis(100)) {
               Ok(ev) => {
                   self.handle_event(ev);
               }
               Err(RecvTimeoutError::Timeout) => {
               }
               Err(err) => {
                   warn!("failed recv from the event queue: {:?}", err);
               }
            }
        }
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

        self.terminal.draw(&*self.current.borrow());
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        let ctrl = KeyModifiers::CONTROL;

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), ctrl) => {
                self.exited = true;
            }
            _ => {
            }
        }
    }
}
