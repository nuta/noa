use crate::syncd_client::SyncdClient;

use crate::view::View;
use anyhow::Context;

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
        }
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

        let opened_file = if let Some(opened_file) = self.files.iter().find(|o| {
            o.read()
                .buffer
                .path()
                .map(|path| path == abspath)
                .unwrap_or(false)
        }) {
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
                let asyncd = self.syncd.clone();
                let opened_file = opened_file.read();
                let path = opened_file.buffer.path().unwrap().to_path_buf();
                let text = opened_file.buffer.text();
                let lang = opened_file.buffer.lang();
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
}
