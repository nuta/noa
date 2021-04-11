use crate::buffer::Buffer;
use crate::terminal::{DrawContext, Terminal};
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use dirs::home_dir;
use log::LevelFilter;
use mpsc::Receiver;
use parking_lot::RwLock;
use simplelog::{Config, WriteLogger};
use std::{
    collections::HashMap,
    env::current_dir,
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::{mpsc::Sender, Arc},
};
use std::{sync::mpsc, time::Instant};
use structopt::StructOpt;

#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    KeyBatch(String),
    NoCompletion,
    Resize {
        screen_height: usize,
        screen_width: usize,
    },
}

#[derive(Clone)]
pub struct EventQueue {
    tx: Sender<Event>,
}

impl EventQueue {
    pub fn new(tx: Sender<Event>) -> EventQueue {
        EventQueue { tx }
    }

    pub fn enqueue(&self, ev: Event) {
        self.tx.send(ev).unwrap();
    }
}

const SCRATCH_TEXT: &str = "\
;; This is the scratch buffer: you can't save it into a file.

fn main() {
    if 1 == 2 {
        println!(\"something went wrong!\");
    }
}
";

pub struct EventLoop {
    exited: bool,
    workspace_dir: PathBuf,
    terminal: Terminal,
    current_buffer: Arc<RwLock<Buffer>>,
    buffers: Vec<Arc<RwLock<Buffer>>>,
    event_queue: Receiver<Event>,
}

impl EventLoop {
    pub fn new(workspace_dir: PathBuf) -> EventLoop {
        let (tx, event_queue) = mpsc::channel();

        let scratch_buffer = Arc::new(RwLock::new(Buffer::from_str(SCRATCH_TEXT)));
        let buffers = vec![scratch_buffer.clone()];

        EventLoop {
            exited: false,
            workspace_dir,
            terminal: Terminal::new(EventQueue::new(tx.clone())),
            event_queue,
            current_buffer: scratch_buffer,
            buffers,
        }
    }

    pub fn open_file(&mut self, path: &Path) {
        let abspath = match path.canonicalize() {
            Ok(abspath) => abspath,
            Err(err) => {
                self.error(format!(
                    "failed to resolve path: {} ({})",
                    path.display(),
                    err
                ));
                return;
            }
        };

        let buffer = match Buffer::open_file(&abspath) {
            Ok(buffer) => Arc::new(RwLock::new(buffer)),
            Err(err) => {
                self.error(format!(
                    "failed to open file: {} ({})",
                    abspath.display(),
                    err
                ));
                return;
            }
        };

        self.buffers.push(buffer.clone());
        self.current_buffer = buffer;
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

                    self.handle_event(ev);
                    while let Ok(ev) = self.event_queue.try_recv() {
                        self.handle_event(ev);
                    }

                    trace!(
                        "event handling took {} us",
                        started_at.elapsed().as_micros()
                    );
                    self.draw();
                }
                Err(err) => {
                    warn!("failed recv from the event queue: {:?}", err);
                }
            }
        }
    }

    pub fn handle_event(&mut self, ev: Event) {
        match ev {
            Event::Key(key) => self.handle_key_event(key),
            _ => {
                trace!("unhandled event = {:?}", ev);
            }
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
        let ctrl_alt = KeyModifiers::CONTROL | KeyModifiers::ALT;

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.exited = true;
            }
            _ => {
                trace!("unhandled key = {:?}", key);
            }
        }
    }

    pub fn draw(&mut self) {
        self.terminal.draw(DrawContext {
            buffer: &*self.current_buffer.read(),
        });
    }

    fn error<T: Into<String>>(&self, message: T) {}
}
