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
use parking_lot::{MappedRwLockWriteGuard, Mutex, MutexGuard, RwLock, RwLockWriteGuard};
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

pub struct BufferManager {
    current_buffer: Arc<RwLock<Buffer>>,
    buffers: Vec<Arc<RwLock<Buffer>>>,
    path2id: HashMap<PathBuf, BufferId>,
    views: HashMap<BufferId, Mutex<View>>,
}

impl BufferManager {
    pub fn new() -> BufferManager {
        let mut scratch = Buffer::from_str(SCRATCH_TEXT);
        scratch.set_name("*scratch*");
        let mut views = HashMap::new();
        views.insert(scratch.id(), Mutex::new(View::new()));
        let scratch_buffer = Arc::new(RwLock::new(scratch));
        let buffers = vec![scratch_buffer.clone()];

        BufferManager {
            current_buffer: scratch_buffer,
            buffers,
            path2id: HashMap::new(),
            views,
        }
    }

    pub fn current_buffer(&self) -> &Arc<RwLock<Buffer>> {
        &self.current_buffer
    }

    pub fn update_current_view_layout(
        &self,
        y_from: usize,
        width: usize,
        height: usize,
    ) -> MutexGuard<View> {
        let buffer = self.current_buffer.read();
        let mut view = self.views[&buffer.id()].lock();
        view.layout(&*buffer, 0, width, height);
        view
    }

    pub async fn open_file(&mut self, path: &Path) -> Result<Arc<RwLock<Buffer>>> {
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
        self.views.insert(buffer_id, Mutex::new(View::new()));
        self.current_buffer = buffer.clone();

        Ok(buffer)
    }
}
