use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::view::View;
use crate::{buffer_manager::BufferManager, surfaces};
use crate::{
    syncd_client::SyncdClient,
    terminal::{self, Terminal},
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
            .call_lsp_method(lang, file_modified_req)
            .await
        {
            warn!("failed to send UpdateFile request: {}", err);
        }

        trace!("sending completion message...");
        if let Err(err) = syncd
            .lock()
            .await
            .call_lsp_method(lang, completion_req)
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
    buffer_manager: BufferManager,
    event_queue: UnboundedReceiver<Event>,
    syncd: Arc<Mutex<SyncdClient>>,
}

impl EventLoop {
    pub fn new(workspace_dir: PathBuf) -> EventLoop {
        let (tx, event_queue) = mpsc::unbounded_channel();

        let buffer_manager = BufferManager::new();
        let workspace_dir = workspace_dir
            .canonicalize()
            .with_context(|| format!("failed to resolve workdir: {}", workspace_dir.display()))
            .unwrap();

        let syncd = SyncdClient::new(&workspace_dir, |noti| {});

        EventLoop {
            exited: false,
            workspace_dir,
            terminal: Terminal::new(EventQueue::new(tx.clone())),
            buffer_manager,
            event_queue,
            syncd: Arc::new(Mutex::new(syncd)),
        }
    }

    pub async fn open_file(&mut self, path: &Path) {
        let buffer = match self.buffer_manager.open_file(path).await {
            Ok(buffer) => buffer,
            Err(err) => {
                self.error(&format!("{}", err));
                return;
            }
        };

        // Tell the LSP server about the newly opened file.
        let asyncd = self.syncd.clone();
        let buffer = buffer.read();
        let path = buffer.path().unwrap().to_path_buf();
        let text = buffer.text();
        let lang = buffer.lang();
        tokio::spawn(async move {
            match asyncd
                .lock()
                .await
                .call_lsp_method::<LspRequest>(lang, LspRequest::OpenFile { path, text })
                .await
            {
                Ok(_) => {}
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
                let prev_ver = self.buffer_manager.current_buffer().read().id_and_version();

                self.handle_event(ev);
                while let Ok(Some(ev)) =
                    timeout(Duration::from_micros(400), self.event_queue.recv()).await
                {
                    self.handle_event(ev);
                }

                let new_ver = self.buffer_manager.current_buffer().read().id_and_version();
                if prev_ver != new_ver {
                    // Switched or modified the current buffer.
                    file_updated_tx.send(self.buffer_manager.current_buffer().clone());
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
            Event::Key(key) => {}
            Event::KeyBatch(str) => {}
            _ => {
                trace!("unhandled event = {:?}", ev);
            }
        }
    }

    pub fn draw(&mut self) {
        self.terminal.draw(&surfaces::Context {
            exited: &mut self.exited,
            buffer_manager: &mut self.buffer_manager,
        });
    }

    fn error<T: Into<String>>(&self, message: T) {
        error!("{}", message.into());
    }
}
