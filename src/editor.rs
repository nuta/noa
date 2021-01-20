use crate::terminal::{KeyEvent, Terminal};
use std::sync::mpsc::{channel, Receiver};

pub enum Event {
    Key(KeyEvent),
    KeyBatch(String),
    Resize { rows: usize, cols: usize },
}

enum EditorMode {
    Normal,
}

pub struct Editor {
    mode: EditorMode,
    terminal: Terminal,
    event_queue: Receiver<Event>,
}

impl Editor {
    pub fn new() -> Editor {
        let (tx, rx) = channel();
        Editor {
            mode: EditorMode::Normal,
            terminal: Terminal::new(tx),
            event_queue: rx,
        }
    }

    pub fn run(&mut self) {}
}
