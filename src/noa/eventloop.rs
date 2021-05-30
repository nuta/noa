use crate::terminal::{DrawContext, Terminal};
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::view::View;
use log::LevelFilter;
use parking_lot::RwLock;
use simplelog::{Config, WriteLogger};
use std::{
    collections::HashMap,
    env::current_dir,
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::Arc,
    task::Poll,
    time::Duration,
};
use std::{sync, time::Instant};
use structopt::StructOpt;
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time::timeout,
};

use noa_buffer::{Buffer, BufferId};

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
    tx: UnboundedSender<Event>,
}

impl EventQueue {
    pub fn new(tx: UnboundedSender<Event>) -> EventQueue {
        EventQueue { tx }
    }

    pub fn enqueue(&self, ev: Event) {
        self.tx.send(ev).ok();
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
    path2id: HashMap<PathBuf, BufferId>,
    views: RwLock<HashMap<BufferId, View>>,
    event_queue: UnboundedReceiver<Event>,
}

impl EventLoop {
    pub fn new(workspace_dir: PathBuf) -> EventLoop {
        let (tx, event_queue) = mpsc::unbounded_channel();

        let mut scratch = Buffer::from_str(SCRATCH_TEXT);
        scratch.set_name("*scratch*");
        let mut views = HashMap::new();
        views.insert(scratch.id(), View::new());
        let scratch_buffer = Arc::new(RwLock::new(scratch));
        let buffers = vec![scratch_buffer.clone()];

        EventLoop {
            exited: false,
            workspace_dir,
            terminal: Terminal::new(EventQueue::new(tx.clone())),
            event_queue,
            current_buffer: scratch_buffer,
            buffers,
            path2id: HashMap::new(),
            views: RwLock::new(views),
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

        let (buffer, buffer_id) = match Buffer::open_file(&abspath) {
            Ok(buffer) => {
                let id = buffer.id();
                (Arc::new(RwLock::new(buffer)), id)
            }
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
        self.path2id.insert(abspath, buffer_id);
        self.current_buffer = buffer;
    }

    pub async fn run(&mut self) {
        self.draw();
        loop {
            if self.exited {
                return;
            }

            if let Some(ev) = self.event_queue.recv().await {
                let started_at = Instant::now();

                self.handle_event(ev);
                while let Ok(Some(ev)) =
                    timeout(Duration::from_micros(400), self.event_queue.recv()).await
                {
                    self.handle_event(ev);
                }

                trace!(
                    "event handling took {} us",
                    started_at.elapsed().as_micros()
                );
                self.draw();
            }
        }
    }

    pub fn handle_event(&mut self, ev: Event) {
        match ev {
            Event::Key(key) => self.handle_key_event(key),
            Event::KeyBatch(str) => {
                self.current_buffer.write().insert(&str);
            }
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
            (KeyCode::Backspace, NONE) => {
                self.current_buffer.write().backspace();
            }
            (KeyCode::Up, NONE) => {
                self.current_buffer.write().move_cursors(1, 0, 0, 0);
            }
            (KeyCode::Down, NONE) => {
                self.current_buffer.write().move_cursors(0, 1, 0, 0);
            }
            (KeyCode::Left, NONE) => {
                self.current_buffer.write().move_cursors(0, 0, 1, 0);
            }
            (KeyCode::Right, NONE) => {
                self.current_buffer.write().move_cursors(0, 0, 0, 1);
            }
            (KeyCode::Enter, NONE) => {
                self.current_buffer.write().insert_char('\n');
            }
            (KeyCode::Char(ch), NONE) => {
                self.current_buffer.write().insert_char(ch);
            }
            _ => {
                trace!("unhandled key = {:?}", key);
            }
        }
    }

    pub fn draw(&mut self) {
        let buffer = self.current_buffer.read();
        let mut views = self.views.write();
        let view = views.get_mut(&buffer.id()).unwrap();
        self.terminal.draw(DrawContext {
            buffer: &*self.current_buffer.read(),
            view,
        });
    }

    fn error<T: Into<String>>(&self, message: T) {}
}
