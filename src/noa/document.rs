use std::{
    collections::HashMap,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Result;

use noa_buffer::buffer::Buffer;
use noa_languages::language::Language;

use crate::view::View;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentId(NonZeroUsize);

pub struct Document {
    /// It's `None` if the document is not backed by a file (e.g. a scrach buffer).
    path: Option<PathBuf>,
    name: String,
    buffer: Buffer,
    lang: &'static Language,
    view: View,
}

impl Document {
    pub fn new(name: &str) -> Result<Document> {
        // let highlighter = Highlighter::new(doc.lang);
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

    pub fn update(&mut self) {
        self.view.update(&self.buffer);
    }
}

pub struct DocumentManager {
    next_document_id: AtomicUsize,
    current: DocumentId,
    documents: HashMap<DocumentId, Document>,
}

impl DocumentManager {
    pub fn new() -> DocumentManager {
        let mut manager = DocumentManager {
            next_document_id: AtomicUsize::new(1),
            current: DocumentId(
                // Safety: 1 is not zero.
                unsafe { NonZeroUsize::new_unchecked(1) },
            ),
            documents: HashMap::new(),
        };

        let scratch_doc = Document::new("**scratch**").unwrap();
        manager.open_virtual_file(scratch_doc);
        manager
    }

    pub fn open_virtual_file(&mut self, doc: Document) {
        // Allocate a document ID.
        let doc_id = DocumentId(
            NonZeroUsize::new(self.next_document_id.fetch_add(1, Ordering::SeqCst)).unwrap(),
        );

        self.documents.insert(doc_id, doc);

        // First run of syntax highlighting, etc.
        self.file_changed(doc_id);

        // Switch to the buffer.
        self.current = doc_id;
    }

    pub fn file_changed(&mut self, document_id: DocumentId) {
        self.documents.get_mut(&document_id).unwrap().update();
    }
}
