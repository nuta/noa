use std::{
    fs::{File, OpenOptions},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Result;
use noa_buffer::{
    buffer::Buffer, cursor::Position, paragraph_iter::ParagraphIndex, reflow_iter::ScreenPosition,
    scroll::Scroll,
};

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
    pub buffer: Buffer,
    pub scroll: Scroll,
}

impl Document {
    pub fn scratch() -> Document {
        Document {
            id: DocumentId::alloc(),
            kind: DocumentKind::Scratch,
            buffer: Buffer::new(),
            scroll: Scroll::zeroed(),
        }
    }

    pub async fn open(path: &Path) -> Result<Document> {
        let file = File::open(path)?;
        Ok(Document {
            id: DocumentId::alloc(),
            kind: DocumentKind::File {
                path: path.to_owned(),
            },
            buffer: Buffer::from_reader(file)?,
            scroll: Scroll::zeroed(),
        })
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
        screen_width: usize,
        screen_height: usize,
        first_visible_pos: Position,
        last_visible_pos: Position,
    ) {
        self.scroll.adjust_scroll(
            &self.buffer,
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
