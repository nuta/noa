use crate::minimap::{LineStatus, MiniMap, MiniMapCategory};
use crate::syncd_client::SyncdClient;

use crate::view::View;
use anyhow::Context;

use noa_common::fast_hash::compute_fast_hash;
use noa_common::oops::OopsExt;
use noa_common::syncd_protocol::lsp_types::DiagnosticSeverity;
use noa_common::syncd_protocol::{FileLocation, LspRequest, Notification};
use parking_lot::RwLock;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

use std::process::Stdio;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::Mutex;

use noa_buffer::{Buffer, BufferId, Point};
use noa_langs::tree_sitter;

const SCRATCH_TEXT: &str = "\
;; This is the scratch buffer: you can't save it into a file.

fn main() {
    if 1 == 2 {
        println!(\"Hello World!\");
    }
}
";

#[derive(Debug, Clone)]
pub enum UserMessage {
    Info(String),
    Error(String),
}

pub struct OpenedFile {
    pub buffer: Buffer,
    pub view: View,
    pub syntax_highlight: Option<tree_sitter::Tree>,
}

impl OpenedFile {
    pub fn layout_view(&mut self, y_from: usize, height: usize, width: usize) {
        self.view.layout(&self.buffer, y_from, height, width);
    }

    pub fn move_cursors(&mut self, y_diff: isize, x_diff: isize) {
        self.view.move_cursors(&mut self.buffer, y_diff, x_diff);
    }

    pub fn expand_selections(&mut self, y_diff: isize, x_diff: isize) {
        self.view
            .expand_selections(&mut self.buffer, y_diff, x_diff);
    }

    pub fn highlight_from_tree_sitter(&mut self) {
        if let Some(ref tree) = self.syntax_highlight {
            self.view
                .highlight_from_tree_sitter(self.buffer.lang(), tree);
        }
    }
}

pub struct Editor {
    exited: bool,
    workspace_dir: PathBuf,
    current_file: Arc<RwLock<OpenedFile>>,
    files: Vec<Arc<RwLock<OpenedFile>>>,
    path2id: HashMap<PathBuf, BufferId>,
    syncd: Arc<Mutex<SyncdClient>>,
    messages: Arc<std::sync::Mutex<Vec<UserMessage>>>,
    jump_list: Vec<FileLocation>,
    minimap: Arc<parking_lot::Mutex<MiniMap>>,
}

impl Editor {
    pub fn new(workspace_dir: &Path, noti_tx: UnboundedSender<Notification>) -> Editor {
        let mut scratch = Buffer::from_str(SCRATCH_TEXT);
        scratch.set_name("*scratch*");
        let scratch_buffer = Arc::new(RwLock::new(OpenedFile {
            buffer: scratch,
            view: View::new(),
            syntax_highlight: None,
        }));

        let files = vec![scratch_buffer.clone()];
        let workspace_dir = workspace_dir
            .canonicalize()
            .with_context(|| format!("failed to resolve workdir: {}", workspace_dir.display()))
            .unwrap();

        let syncd = SyncdClient::new(&workspace_dir, noti_tx);

        Editor {
            exited: false,
            workspace_dir,
            current_file: scratch_buffer,
            files,
            path2id: HashMap::new(),
            syncd: Arc::new(Mutex::new(syncd)),
            messages: Arc::new(std::sync::Mutex::new(Vec::new())),
            jump_list: Vec::new(),
            minimap: Arc::new(parking_lot::Mutex::new(MiniMap::new())),
        }
    }

    pub fn minimap(&self) -> &Arc<parking_lot::Mutex<MiniMap>> {
        &self.minimap
    }

    pub fn exited(&self) -> bool {
        self.exited
    }

    pub fn exit_editor(&mut self) {
        self.exited = true;
    }

    pub fn last_message(&self) -> Option<UserMessage> {
        self.messages.lock().unwrap().last().cloned()
    }

    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    pub fn syncd(&self) -> &Arc<Mutex<SyncdClient>> {
        &self.syncd
    }

    pub fn current_file(&self) -> &Arc<RwLock<OpenedFile>> {
        &self.current_file
    }

    pub fn log(&self, m: UserMessage) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(m);
        messages.truncate(128);
    }

    pub fn info<T: Into<String>>(&self, str: T) {
        let string = str.into();
        info!("info: {}", string);
        self.log(UserMessage::Info(string));
    }

    pub fn error<T: Into<String>>(&self, str: T) {
        let string = str.into();
        error!("error: {}", string);
        self.log(UserMessage::Error(string));
    }

    pub fn get_opened_file_by_path(&mut self, path: &Path) -> Option<&Arc<RwLock<OpenedFile>>> {
        self.files
            .iter()
            .find(|o| o.read().buffer.path().map(|p| p == path).unwrap_or(false))
    }

    pub fn open_file(&mut self, path: &Path, cursor_pos: Option<Point>) {
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

        let opened_file = if let Some(opened_file) = self.get_opened_file_by_path(&abspath) {
            // The path is already opened.
            opened_file.clone()
        } else {
            // The file is not yet opened.
            let (buffer, buffer_id) = match Buffer::open_file(&abspath) {
                Ok(buffer) => {
                    let id = buffer.id();
                    (buffer, id)
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

            let opened_file = Arc::new(RwLock::new(OpenedFile {
                buffer,
                view: View::new(),
                syntax_highlight: None,
            }));

            self.files.push(opened_file.clone());
            self.path2id.insert(abspath.clone(), buffer_id);

            {
                // Tell the LSP server about the newly opened file.
                let syncd = self.syncd.clone();
                let opened_file = opened_file.clone();
                tokio::spawn(async move {
                    syncd
                        .lock()
                        .await
                        .call_lsp_method_for_file(&opened_file, |path, opened_file| {
                            LspRequest::OpenFile {
                                path,
                                text: opened_file.buffer.text(),
                            }
                        })
                        .await
                        .oops();
                });
            }

            {
                // Tell the buffer-sync server about the newly opened file.
                let path = abspath.clone();
                let syncd = self.syncd.clone();
                tokio::spawn(async move {
                    syncd.lock().await.call_buffer_open_file(&path).await.oops();
                });
            }

            opened_file
        };

        self.current_file = opened_file.clone();

        // Move the cursor to the specified position.
        let cursor_pos = cursor_pos.unwrap_or(Point::new(0, 0));
        opened_file.write().buffer.move_cursor_to(cursor_pos);
        self.jump_list.push(FileLocation {
            path: abspath,
            pos: cursor_pos,
        });
    }

    pub fn save_current_buffer(&self) {
        if let Err(err) = self.current_file.write().buffer.save() {
            self.error(format!("{}", err));
        }
    }

    pub fn dirty_buffers(&self) -> Vec<Arc<RwLock<OpenedFile>>> {
        let mut buffers = Vec::new();
        for opened_file_lock in &self.files {
            let opened_file = opened_file_lock.read();
            let buffer = &opened_file.buffer;
            if buffer.is_dirty() && !buffer.is_virtual_file() {
                buffers.push(opened_file_lock.clone());
            }
        }
        buffers
    }

    pub fn save_all(&self) {
        for opened_file in self.dirty_buffers() {
            if let Err(err) = opened_file.write().buffer.save() {
                self.error(format!("{}", err));
            }
        }
    }

    pub fn check_run_background(&mut self, title: &str, cmd: &mut Command) {
        let proc = match cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(proc) => proc,
            Err(err) => {
                error!("failed to execute {}: {}", title, err);
                self.error(format!("{}: {}", title, err));
                return;
            }
        };

        let messages = self.messages.clone();
        let title = title.to_owned();
        tokio::spawn(async move {
            match proc.wait_with_output().await {
                Ok(result) => {
                    let mut messages = messages.lock().unwrap();
                    match std::str::from_utf8(&result.stderr) {
                        Ok(stderr) => {
                            warn!("stderr ({}): {}", title, stderr);
                            messages.push(UserMessage::Error(stderr.to_owned()));
                        }
                        Err(err) => {
                            error!("failed to execute {}: {}", title, err);
                            messages
                                .push(UserMessage::Error(format!("{}: non-utf8 stderr", title)));
                        }
                    };
                }
                Err(err) => {
                    let mut messages = messages.lock().unwrap();
                    error!("failed to execute {}: {}", title, err);
                    messages.push(UserMessage::Error(format!("{}: {}", title, err)));
                }
            }
        });
    }

    pub fn handle_sync_notification(&mut self, noti: Notification) {
        match noti {
            Notification::FileModified {
                path,
                text,
                hash: fast_hash,
            } => {
                // TODO:
                trace!("file modified: {}\n{}", path.display(), text);
                if let Some(opened_file) = self.get_opened_file_by_path(&path) {
                    let mut f = opened_file.write();
                    if compute_fast_hash(f.buffer.text().as_bytes()) != fast_hash {
                        f.buffer.set_text(&text);
                    }
                }
            }
            Notification::Diagnostics(diags) => {
                // TODO: Check if the path is current one.
                let mut minimap = self.minimap.lock();
                minimap.clear(MiniMapCategory::Diagnosis);
                for diag in diags {
                    trace!("diagnostic: {:?}", diag);
                    let interval =
                        (diag.range.start.line as usize)..(diag.range.end.line as usize + 1);
                    match diag.severity {
                        Some(DiagnosticSeverity::Error) => {
                            minimap.insert(MiniMapCategory::Diagnosis, interval, LineStatus::Error);
                        }
                        Some(DiagnosticSeverity::Warning) => {
                            minimap.insert(
                                MiniMapCategory::Diagnosis,
                                interval,
                                LineStatus::Warning,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
