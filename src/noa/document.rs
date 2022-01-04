use std::{
    collections::HashMap,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::atomic::AtomicUsize,
};

use anyhow::Result;

use noa_buffer::buffer::Buffer;
use noa_languages::{highlighting::Highlighter, language::Language};

use crate::view::View;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentId(NonZeroUsize);

pub struct Document {
    /// It's `None` if the document is not backed by a file (e.g. a scrach buffer).
    path: Option<PathBuf>,
    name: String,
    buffer: Buffer,
    lang: &'static Language,
}

impl Document {
    pub fn new(name: &str) -> Result<Document> {
        unimplemented!()
    }

    pub fn open_file(path: &Path) -> Result<Document> {
        unimplemented!()
    }

    pub fn save_to_file(&self) -> Result<()> {
        if let Some(ref path) = self.path {
            self.buffer.save_to_file(path)?;
        }

        Ok(())
    }
}

pub struct DocumentManager {
    next_document_id: AtomicUsize,
    documents: HashMap<DocumentId, Document>,
    views: HashMap<DocumentId, View>,
    highlighters: HashMap<DocumentId, Highlighter>,
}

impl DocumentManager {
    pub fn new() -> DocumentManager {
        DocumentManager {
            next_document_id: AtomicUsize::new(1),
            documents: HashMap::new(),
            views: HashMap::new(),
            highlighters: HashMap::new(),
        }
    }

    pub fn file_changed(&mut self) {
        let document_id: DocumentId = unimplemented!();
        let rope = self.documents[&document_id].buffer.raw_buffer().rope();
        self.highlighters[&document_id].update(rope);
    }
}
