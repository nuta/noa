use crate::git::DiffType;
use crate::minimap::{LineStatus, MiniMap, MiniMapCategory};
use crate::sync_client::SyncClient;

use crate::view::View;
use anyhow::{Context, Result};

use noa_common::fast_hash::compute_fast_hash;
use noa_common::oops::OopsExt;
use noa_common::sync_protocol::lsp_types::DiagnosticSeverity;
use noa_common::sync_protocol::{FileLocation, LspRequest, Notification};
use noa_common::tmux::{self};
use parking_lot::RwLock;
use tokio::process::Command;

use std::path::Path;
use std::process::Stdio;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use tokio::sync::Mutex;

use noa_buffer::{Buffer, BufferId, Point};
use noa_langs::tree_sitter;

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

const SCRATCH_TEXT: &str = "\
;; This is the scratch buffer: you can't save it into a file.

fn main() {
    if 1 == 2 {
        println!(\"Hello World!\");
    }
}
";

pub struct BufferSet {
    current_file: Arc<RwLock<OpenedFile>>,
    files: Vec<Arc<RwLock<OpenedFile>>>,
    path2id: HashMap<PathBuf, BufferId>,
}

impl BufferSet {
    pub fn new() -> BufferSet {
        let mut scratch = Buffer::from_str(SCRATCH_TEXT);
        scratch.set_name("*scratch*");
        let scratch_buffer = Arc::new(RwLock::new(OpenedFile {
            buffer: scratch,
            view: View::new(),
            syntax_highlight: None,
        }));

        let files = vec![scratch_buffer.clone()];

        BufferSet {
            current_file: scratch_buffer,
            files,
            path2id: HashMap::new(),
        }
    }

    pub fn current_file(&self) -> &Arc<RwLock<OpenedFile>> {
        &self.current_file
    }

    pub fn get_opened_file_by_path(&mut self, path: &Path) -> Option<&Arc<RwLock<OpenedFile>>> {
        self.files
            .iter()
            .find(|o| o.read().buffer.path().map(|p| p == path).unwrap_or(false))
    }

    pub fn open_file(&mut self, path: &Path, cursor_pos: Option<Point>) -> Result<()> {
        let abspath = path.canonicalize()?;
        let opened_file = if let Some(opened_file) = self.get_opened_file_by_path(&abspath) {
            // The path is already opened.
            opened_file.clone()
        } else {
            // The file is not yet opened.
            let buffer = Buffer::open_file(&abspath)?;
            let buffer_id = buffer.id();
            let opened_file = Arc::new(RwLock::new(OpenedFile {
                buffer,
                view: View::new(),
                syntax_highlight: None,
            }));

            self.files.push(opened_file.clone());
            self.path2id.insert(abspath.clone(), buffer_id);

            opened_file
        };

        self.current_file = opened_file.clone();

        // Move the cursor to the specified position.
        let cursor_pos = cursor_pos.unwrap_or(Point::new(0, 0));
        opened_file.write().buffer.move_cursor_to(cursor_pos);

        Ok(())
    }

    fn format_and_save(&self, buffer: &mut Buffer) -> Result<()> {
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

        buffer.save()?;
        Ok(())
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

    pub fn check_run_background(&mut self, title: &str, cmd: &mut Command) -> Result<()> {
        let proc = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        let title = title.to_owned();
        tokio::spawn(async move {
            match proc.wait_with_output().await {
                Ok(result) => {
                    match std::str::from_utf8(&result.stderr) {
                        Ok(stderr) => {
                            // TODO: message
                            warn!("stderr ({}): {}", title, stderr);
                        }
                        Err(err) => {
                            // TODO: message
                            error!("failed to execute {}: {}", title, err);
                        }
                    };
                }
                Err(err) => {
                    error!("failed to execute {}: {}", title, err);
                    // TODO: message
                }
            }
        });

        Ok(())
    }

    /*
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

    */
}
