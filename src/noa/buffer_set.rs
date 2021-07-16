use crate::sync_client::SyncClient;
use crate::view::View;
use crate::Event;
use anyhow::Result;

use noa_common::oops::OopsExt;
use noa_common::sync_protocol::{LspRequest, LspResponse};
use parking_lot::RwLock;
use tokio::sync::mpsc::UnboundedSender;

use std::path::Path;
use std::process::Stdio;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use noa_buffer::{Buffer, BufferId, Cursor, Point};
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

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.view.set_cursor(&mut self.buffer, cursor);
    }

    pub fn move_cursors(&mut self, y_diff: isize, x_diff: isize) {
        self.view.move_cursors(&mut self.buffer, y_diff, x_diff);
    }

    pub fn scroll(&mut self, y_diff: isize) {
        self.view.scroll(&mut self.buffer, y_diff);
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

    pub fn open_file(
        &mut self,
        sync: &Arc<SyncClient>,
        event_tx: &UnboundedSender<Event>,
        path: &Path,
        cursor_pos: Option<Point>,
    ) -> Result<()> {
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

        // Compute syntax highlighting.
        {
            let event_tx = event_tx.clone();
            let opened_file = opened_file.clone();
            tokio::spawn(async move {
                let (rope, mut parser) = {
                    let f = opened_file.read();
                    let rope = f.buffer.rope().clone();
                    let parser = match f.buffer.lang().syntax_highlighting_parser() {
                        Some(parser) => parser,
                        None => return,
                    };
                    (rope, parser)
                };

                if let Some(tree) = parser.parse(rope.text(), None) {
                    opened_file.write().syntax_highlight = Some(tree);
                }

                event_tx.send(Event::ReDraw).ok();
            });
        }

        // Tell the LSP server about the newly opened file.
        {
            let sync = sync.clone();
            let opened_file = opened_file.clone();
            tokio::spawn(async move {
                sync.call_lsp_method_for_file(
                    &opened_file,
                    |path, opened_file| LspRequest::OpenFile {
                        path,
                        text: opened_file.buffer.text(),
                    },
                    |_: LspResponse| Ok(()),
                )
                .await
                .oops();
            });
        }

        // Tell the buffer-sync server about the newly opened file.
        {
            let path = abspath.clone();
            let sync = sync.clone();
            tokio::spawn(async move {
                sync.call_buffer_open_file(&path).await.oops();
            });
        }

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

    pub fn save_current_buffer(&self) -> Result<()> {
        self.format_and_save(&mut self.current_file.write().buffer)?;
        Ok(())
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

    pub fn save_all(&self) -> Result<()> {
        for opened_file in self.dirty_buffers() {
            self.format_and_save(&mut opened_file.write().buffer)?;
        }

        Ok(())
    }
}
