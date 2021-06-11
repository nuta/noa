use crate::syncd_client::SyncdClient;
use crate::view::View;
use anyhow::{bail, Context, Result};

use noa_common::syncd_protocol::LspRequest;
use parking_lot::RwLock;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::Mutex;

use noa_buffer::{Buffer, BufferId};

const SCRATCH_TEXT: &str = "\
;; This is the scratch buffer: you can't save it into a file.

fn main() {
    if 1 == 2 {
        println!(\"Hello World!\");
    }
}
";

enum UserMessage {
    Error(String),
}

pub struct Editor {
    exited: bool,
    workspace_dir: PathBuf,
    current_buffer: Arc<RwLock<Buffer>>,
    buffers: Vec<Arc<RwLock<Buffer>>>,
    path2id: HashMap<PathBuf, BufferId>,
    views: HashMap<BufferId, parking_lot::Mutex<View>>,
    syncd: Arc<Mutex<SyncdClient>>,
    messages: parking_lot::Mutex<Vec<UserMessage>>,
}

impl Editor {
    pub fn new(workspace_dir: PathBuf) -> Editor {
        let mut scratch = Buffer::from_str(SCRATCH_TEXT);
        scratch.set_name("*scratch*");
        let mut views = HashMap::new();
        views.insert(scratch.id(), parking_lot::Mutex::new(View::new()));
        let scratch_buffer = Arc::new(RwLock::new(scratch));

        let buffers = vec![scratch_buffer.clone()];
        let workspace_dir = workspace_dir
            .canonicalize()
            .with_context(|| format!("failed to resolve workdir: {}", workspace_dir.display()))
            .unwrap();

        let syncd = SyncdClient::new(&workspace_dir, |_noti| {});

        Editor {
            exited: false,
            workspace_dir,
            current_buffer: scratch_buffer,
            buffers,
            path2id: HashMap::new(),
            views,
            syncd: Arc::new(Mutex::new(syncd)),
            messages: parking_lot::Mutex::new(Vec::new()),
        }
    }

    pub fn exited(&self) -> bool {
        self.exited
    }

    pub fn exit_editor(&mut self) {
        self.exited = true;
    }

    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    pub fn syncd(&self) -> &Arc<Mutex<SyncdClient>> {
        &self.syncd
    }

    pub fn current_buffer(&self) -> &Arc<RwLock<Buffer>> {
        &self.current_buffer
    }

    pub fn view(&self, buffer: &Buffer) -> parking_lot::MutexGuard<'_, View> {
        self.views[&buffer.id()].lock()
    }

    pub fn error<T: Into<String>>(&self, str: T) {
        let string = str.into();
        error!("error: {}", string);
        let mut messages = self.messages.lock();
        messages.push(UserMessage::Error(string));
        messages.truncate(128);
    }

    pub fn compute_view(
        &self,
        buffer: &Buffer,
        width: usize,
        height: usize,
    ) -> parking_lot::MutexGuard<'_, View> {
        let mut view = self.views[&buffer.id()].lock();
        view.layout(buffer, 0, width, height);
        view
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
        self.views
            .insert(buffer_id, parking_lot::Mutex::new(View::new()));
        self.current_buffer = buffer.clone();
        /* TODO:
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
        */
    }
}
