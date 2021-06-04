use crate::surfaces;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::view::View;
use crate::{
    syncd_client::SyncdClient,
    terminal::{self, Terminal},
};
use anyhow::{bail, Context, Result};
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
use tokio::sync::{Mutex, MutexGuard};
use tokio::{
    sync::mpsc::{self, unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::timeout,
};

use noa_buffer::{Buffer, BufferId};

const SCRATCH_TEXT: &str = "\
;; This is the scratch buffer: you can't save it into a file.

fn main() {
    if 1 == 2 {
        println!(\"Hello World!\");
    }
}
";

pub struct Editor {
    exited: bool,
    workspace_dir: PathBuf,
    current_buffer: Arc<RwLock<Buffer>>,
    buffers: Vec<Arc<RwLock<Buffer>>>,
    path2id: HashMap<PathBuf, BufferId>,
    views: HashMap<BufferId, parking_lot::Mutex<View>>,
    syncd: Arc<Mutex<SyncdClient>>,
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

        let syncd = SyncdClient::new(&workspace_dir, |noti| {});

        Editor {
            exited: false,
            workspace_dir,
            current_buffer: scratch_buffer,
            buffers,
            path2id: HashMap::new(),
            views,
            syncd: Arc::new(Mutex::new(syncd)),
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

    pub async fn open_file(&mut self, path: &Path) -> Result<()> {
        let abspath = match path.canonicalize() {
            Ok(abspath) => abspath,
            Err(err) => {
                bail!("failed to resolve path: {} ({})", path.display(), err);
            }
        };

        let (buffer, buffer_id) = match Buffer::open_file(&abspath) {
            Ok(buffer) => {
                let id = buffer.id();
                (Arc::new(RwLock::new(buffer)), id)
            }
            Err(err) => {
                bail!("failed to open file: {} ({})", abspath.display(), err);
            }
        };

        self.buffers.push(buffer.clone());
        self.path2id.insert(abspath.clone(), buffer_id);
        self.views
            .insert(buffer_id, parking_lot::Mutex::new(View::new()));
        self.current_buffer = buffer.clone();

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

        Ok(())
    }
}
