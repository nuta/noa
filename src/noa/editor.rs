use crate::git::{compute_line_diffs, resolve_git_dir, DiffType};
use crate::minimap::{LineStatus, MiniMap, MiniMapCategory};
use crate::sync_client::SyncClient;

use crate::view::View;
use anyhow::Context;

use noa_common::fast_hash::compute_fast_hash;
use noa_common::oops::OopsExt;
use noa_common::sync_protocol::lsp_types::DiagnosticSeverity;
use noa_common::sync_protocol::{FileLocation, LspRequest, Notification};
use noa_common::tmux::{self};
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

    pub fn highlight_by_find_all(&mut self, needle: &str) {
        self.view
            .set_search_highlights(self.buffer.find_all(needle, None));
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
    git_dir: Option<PathBuf>,
    workspace_dir: PathBuf,
    current_file: Arc<RwLock<OpenedFile>>,
    files: Vec<Arc<RwLock<OpenedFile>>>,
    path2id: HashMap<PathBuf, BufferId>,
    sync: Arc<Mutex<SyncClient>>,
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

        let sync = SyncClient::new(&workspace_dir, noti_tx);

        Editor {
            exited: false,
            git_dir: resolve_git_dir(&workspace_dir),
            workspace_dir,
            current_file: scratch_buffer,
            files,
            path2id: HashMap::new(),
            sync: Arc::new(Mutex::new(sync)),
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

    pub fn sync(&self) -> &Arc<Mutex<SyncClient>> {
        &self.sync
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
                let sync = self.sync.clone();
                let opened_file = opened_file.clone();
                tokio::spawn(async move {
                    sync.lock()
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
                let sync = self.sync.clone();
                tokio::spawn(async move {
                    sync.lock().await.call_buffer_open_file(&path).await.oops();
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

    fn format_and_save(&self, buffer: &mut Buffer) {
        // Format the file.
        if let Some(argv) = buffer.lang().formatter {
            trace!("formatting with {:?}", argv);
            let child = std::process::Command::new(argv[0])
                .args(&argv[1..])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            match child {
                Ok(mut child) => {
                    use std::io::Write;

                    let mut stdin = child.stdin.take().unwrap();
                    stdin.write(buffer.text().as_bytes()).ok();
                    drop(stdin);

                    match child.wait_with_output() {
                        Ok(output) if output.status.success() => {
                            match std::str::from_utf8(&output.stdout) {
                                Ok(text) => {
                                    buffer.mark_undo_point();
                                    buffer.set_text(text);
                                }
                                Err(err) => {
                                    error!("{} generated non-UTF8 text: {:?}", argv[0], err);
                                }
                            }
                        }
                        Ok(output) => {
                            self.error("formatting error (ignored)");
                            error!(
                                "formatter error: {}:\nstdout: {}",
                                argv[0],
                                std::str::from_utf8(&output.stderr).unwrap()
                            );
                        }
                        Err(err) => {
                            error!("formatter error: {}: {:?}", argv[0], err);
                        }
                    }
                }
                Err(err) => {
                    error!("failed to execute {:?}: {:?}", argv, err);
                }
            }
        }

        if let Err(err) = buffer.save() {
            self.error(format!("{}", err));
        }
    }

    pub fn save_current_buffer(&self) {
        self.format_and_save(&mut self.current_file.write().buffer);
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
            self.format_and_save(&mut opened_file.write().buffer);
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
            Notification::FileModified { path, text, hash } => {
                trace!("file change detected: {}", path.display());
                if let Some(opened_file) = self.get_opened_file_by_path(&path) {
                    let mut f = opened_file.write();
                    if compute_fast_hash(f.buffer.text().as_bytes()) != hash {
                        f.buffer.set_text(&text);
                    }
                }
            }
            Notification::Diagnostics { path, diags } => {
                if Some(path.as_path()) == self.current_file.read().buffer.path() {
                    let mut minimap = self.minimap.lock();
                    minimap.clear(MiniMapCategory::Diagnosis);
                    for diag in diags {
                        trace!("diagnostic: {:?}", diag);
                        let interval =
                            (diag.range.start.line as usize)..(diag.range.end.line as usize + 1);
                        match diag.severity {
                            Some(DiagnosticSeverity::Error) => {
                                minimap.insert(
                                    MiniMapCategory::Diagnosis,
                                    interval,
                                    LineStatus::Error,
                                );
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
            Notification::OpenFileInOther {
                pane_id,
                path,
                position,
            } => match tmux::get_this_tmux_pane_id() {
                Some(our_pane_id) if our_pane_id == pane_id => {
                    self.info("opened a file");
                    self.open_file(&path, position);
                    tmux::select_pane(our_pane_id).oops();
                }
                _ => {}
            },
        }
    }

    pub fn update_git_line_statuses(&self) {
        let git_dir = self.git_dir.to_owned();
        let minimap = self.minimap.clone();
        let current_file = self.current_file.clone();
        tokio::spawn(async move {
            let (git_dir, path, text) = {
                let f = current_file.read();
                match (git_dir, f.buffer.path()) {
                    (Some(dir), Some(path)) => (dir, path.to_owned(), f.buffer.text()),
                    _ => return,
                }
            };

            let diffs = match compute_line_diffs(&git_dir, &path, &text) {
                Ok(diffs) => diffs,
                Err(err) => {
                    trace!("failed to get git diff: {:?}", err);
                    return;
                }
            };

            let mut minimap = minimap.lock();
            minimap.clear(MiniMapCategory::Diff);
            for diff in diffs {
                trace!(
                    "git diff: range={:?}, type={:?}",
                    diff.range,
                    diff.diff_type
                );

                let value = match diff.diff_type {
                    DiffType::Added => LineStatus::AddedLine,
                    DiffType::Removed => LineStatus::RemovedLine,
                    DiffType::Modified => LineStatus::ModifiedLine,
                };

                minimap.insert(MiniMapCategory::Diff, diff.range, value);
            }
        });
    }
}
