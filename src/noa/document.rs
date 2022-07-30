use std::{
    fs::{File, OpenOptions},
    io::ErrorKind,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};

use anyhow::Result;
use noa_buffer::{
    buffer::Buffer, cursor::Position, paragraph_iter::ParagraphIndex, raw_buffer::RawBuffer,
    reflow_iter::ScreenPosition, scroll::Scroll,
};

use crate::{notify_info, notify_warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(usize);

impl DocumentId {
    pub fn alloc() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        DocumentId(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }
}

#[derive(Debug)]
pub enum DocumentKind {
    Scratch,
    File { path: PathBuf },
}

pub struct Document {
    pub id: DocumentId,
    pub kind: DocumentKind,
    pub name: String,
    pub buffer: Buffer,
    pub saved_buffer: RawBuffer,
    last_saved_at: Option<SystemTime>,
    pub path: Option<PathBuf>,
    pub backup_path: Option<PathBuf>,
    pub scroll: Scroll,
}

impl Document {
    pub fn scratch() -> Document {
        let buffer = Buffer::new();
        let saved_buffer = buffer.raw_buffer().clone();
        Document {
            name: "[scratch]".to_string(),
            id: DocumentId::alloc(),
            kind: DocumentKind::Scratch,
            buffer,
            saved_buffer,
            last_saved_at: None,
            path: None,
            backup_path: None,
            scroll: Scroll::zeroed(),
        }
    }

    pub async fn open(path: &Path) -> Result<Document> {
        let file = File::open(path)?;
        let buffer = Buffer::from_reader(file)?;
        let saved_buffer = buffer.raw_buffer().clone();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        Ok(Document {
            id: DocumentId::alloc(),
            name,
            kind: DocumentKind::File {
                path: path.to_owned(),
            },
            buffer,
            saved_buffer,
            last_saved_at: None,
            path: Some(path.to_owned()),
            backup_path: None, // TODO:
            scroll: Scroll::zeroed(),
        })
    }

    fn do_save_to_file(&mut self) {
        let path = match &self.path {
            Some(path) => path,
            None => return,
        };

        trace!("saving into a file: {}", path.display());
        let with_sudo = match self.buffer.save_to_file(path) {
            Ok(()) => {
                if let Some(backup_path) = &self.backup_path {
                    let _ = std::fs::remove_file(backup_path);
                }

                false
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                match self.buffer.save_to_file_with_sudo(path) {
                    Ok(()) => {
                        if let Some(backup_path) = &self.backup_path {
                            let _ = std::fs::remove_file(backup_path);
                        }

                        true
                    }
                    Err(err) => {
                        notify_warn!("failed to save: {}", err);
                        return;
                    }
                }
            }
            Err(err) => {
                notify_warn!("failed to save: {}", err);
                return;
            }
        };

        self.saved_buffer = self.buffer.raw_buffer().clone();

        // FIXME: By any chance, the file was modified by another process
        // between saving the file and updating the last saved time here.
        //
        // For now, we just ignore that case.
        match std::fs::metadata(path).and_then(|meta| meta.modified()) {
            Ok(modified) => {
                self.last_saved_at = Some(modified);
            }
            Err(err) => {
                notify_warn!("failed to get last saved time: {}", err);
            }
        }

        notify_info!(
            "written {} lines{}",
            self.buffer.num_lines(),
            if with_sudo { " w/ sudo" } else { "" }
        );
    }

    pub fn is_dirty(&self) -> bool {
        let a = self.buffer.raw_buffer();
        let b = &self.saved_buffer;

        a.len_chars() != b.len_chars() && a != b
    }

    pub fn scroll_down(&mut self, n: usize, screen_width: usize) {
        self.scroll.scroll_down(
            &self.buffer,
            screen_width,
            self.buffer.editorconfig().tab_width,
            n,
        );
    }

    pub fn scroll_up(&mut self, n: usize, screen_width: usize) {
        self.scroll.scroll_up(
            &self.buffer,
            screen_width,
            self.buffer.editorconfig().tab_width,
            n,
        );
    }

    pub fn adjust_scroll(
        &mut self,
        virtual_screen_width: usize,
        screen_width: usize,
        screen_height: usize,
        first_visible_pos: Position,
        last_visible_pos: Position,
    ) {
        self.scroll.adjust_scroll(
            &self.buffer,
            virtual_screen_width,
            screen_width,
            screen_height,
            self.buffer.editorconfig().tab_width,
            first_visible_pos,
            last_visible_pos,
            self.buffer.main_cursor().moving_position(),
        );
    }
}

impl Deref for Document {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for Document {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}
