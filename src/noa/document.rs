use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use noa_buffer::{buffer::Buffer, cursor::Position, reflow_iter::ScreenPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(usize);

impl DocumentId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        DocumentId(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }
}

#[derive(Debug)]
pub enum DocumentKind {
    Scratch,
    File { path: PathBuf },
}

#[derive(Debug)]
pub struct Scroll {
    pub buf_pos: Position,
    pub screen_pos: ScreenPosition,
}

pub struct Document {
    pub id: DocumentId,
    pub kind: DocumentKind,
    pub buffer: Buffer,
    pub scroll: Scroll,
}

impl Document {
    pub fn scratch() -> Self {
        Document {
            id: DocumentId::new(),
            kind: DocumentKind::Scratch,
            buffer: Buffer::new(),
            scroll: Scroll {
                buf_pos: Position::new(0, 0),
                screen_pos: ScreenPosition::new(0, 0),
            },
        }
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
