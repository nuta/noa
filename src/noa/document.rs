use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use noa_buffer::buffer::Buffer;

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

pub struct Document {
    pub id: DocumentId,
    kind: DocumentKind,
    buffer: Buffer,
}

impl Document {
    pub fn scratch() -> Self {
        Document {
            id: DocumentId::new(),
            kind: DocumentKind::Scratch,
            buffer: Buffer::new(),
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
