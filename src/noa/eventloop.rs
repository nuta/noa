use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::view::View;
use crate::{
    syncd::SyncdClient,
    terminal::{DrawContext, Terminal},
};
use anyhow::Context;
use log::LevelFilter;
use noa_common::{
    syncd_protocol::{LspRequest, LspResponse},
    warn_on_error,
};
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
    sync::{
        mpsc::{self, unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
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
        println!(\"Hello World!\");
    }
}
";

async fn file_updated_handler(
    mut rx: UnboundedReceiver<Arc<RwLock<Buffer>>>,
    workspace_dir: PathBuf,
    syncd: Arc<Mutex<SyncdClient>>,
) {
    while let Some(buffer_lock) = rx.recv().await {
        let (lang, file_modified_req, completion_req) = {
            let buffer = buffer_lock.read();
            let (path) = match buffer.path() {
                Some(path) => path,
                None => {
                    continue;
                }
            };

            // Ignore files that're not under the workspace directory.
            if !path.starts_with(&workspace_dir) {
                continue;
            }

            let lang = buffer.lang();
            let file_modified_req = LspRequest::UpdateFile {
                path: path.to_owned(),
                version: buffer.version(),
                text: buffer.text(),
            };

            let completion_req = LspRequest::Completion {
                path: path.to_owned(),
                position: *buffer.main_cursor_pos(),
            };

            (lang, file_modified_req, completion_req)
        };

        if let Err(err) = syncd
            .lock()
            .await
            .send_lsp_message(lang, file_modified_req)
            .await
        {
            warn!("failed to send UpdateFile request: {}", err);
        }

        trace!("sending completion message...");
        if let Err(err) = syncd
            .lock()
            .await
            .send_lsp_message(lang, completion_req)
            .await
        {
            warn!("failed to call Completion request: {}", err);
        }
    }

    trace!("exiting file update handler");
}

pub struct EventLoop {
    exited: bool,
    workspace_dir: PathBuf,
    terminal: Terminal,
    current_buffer: Arc<RwLock<Buffer>>,
    buffers: Vec<Arc<RwLock<Buffer>>>,
    path2id: HashMap<PathBuf, BufferId>,
    views: RwLock<HashMap<BufferId, View>>,
    event_queue: UnboundedReceiver<Event>,
    syncd: Arc<Mutex<SyncdClient>>,
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

        let workspace_dir = workspace_dir
            .canonicalize()
            .with_context(|| format!("failed to resolve workdir: {}", workspace_dir.display()))
            .unwrap();

        let syncd = SyncdClient::new(&workspace_dir, |noti| {});

        EventLoop {
            exited: false,
            workspace_dir,
            terminal: Terminal::new(EventQueue::new(tx.clone())),
            event_queue,
            current_buffer: scratch_buffer,
            buffers,
            path2id: HashMap::new(),
            views: RwLock::new(views),
            syncd: Arc::new(Mutex::new(syncd)),
        }
    }

    pub async fn open_file(&mut self, path: &Path) {
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

        let (buffer, buffer_id, lang, text) = match Buffer::open_file(&abspath) {
            Ok(buffer) => {
                let id = buffer.id();
                let lang = buffer.lang();
                let text = buffer.text();
                (Arc::new(RwLock::new(buffer)), id, lang, text)
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
        self.path2id.insert(abspath.clone(), buffer_id);
        self.views.write().insert(buffer_id, View::new());
        self.current_buffer = buffer.clone();

        // Tell the LSP server about the newly opened file.
        let asyncd = self.syncd.clone();
        tokio::spawn(async move {
            match asyncd
                .lock()
                .await
                .send_lsp_message::<LspRequest>(
                    lang,
                    LspRequest::OpenFile {
                        path: abspath,
                        text,
                    },
                )
                .await
            {
                Ok(()) => {}
                Err(err) => {
                    warn!("failed to send a syncd request: {}", err);
                }
            };
        });
    }

    pub async fn run(&mut self) {
        // Register the event handler on file updates.
        let (file_updated_tx, mut file_updated_rx) = unbounded_channel::<Arc<RwLock<Buffer>>>();
        tokio::spawn(file_updated_handler(
            file_updated_rx,
            self.workspace_dir.clone(),
            self.syncd.clone(),
        ));

        self.draw();
        loop {
            if self.exited {
                return;
            }

            if let Some(ev) = self.event_queue.recv().await {
                let started_at = Instant::now();
                let prev_ver = self.current_buffer.read().id_and_version();

                self.handle_event(ev);
                while let Ok(Some(ev)) =
                    timeout(Duration::from_micros(400), self.event_queue.recv()).await
                {
                    self.handle_event(ev);
                }

                let new_ver = self.current_buffer.read().id_and_version();
                if prev_ver != new_ver {
                    // Switched or modified the current buffer.
                    file_updated_tx.send(self.current_buffer.clone());
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

    fn error<T: Into<String>>(&self, message: T) {
        error!("{}", message.into());
    }
}
