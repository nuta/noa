use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::Duration;
use crate::buffer::Buffer;
use crate::view::View;
use crate::terminal::{Terminal, KeyCode, KeyModifiers, KeyEvent};

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
        }
    }

    pub fn run(&mut self) {
        loop {
            if self.exited {
                return;
            }

            self.terminal.draw(&*self.current.borrow());
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
