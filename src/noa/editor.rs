use std::collections::HashMap;

use crate::{
    clipboard::{self, ClipboardProvider},
    document::{Document, DocumentId},
};

pub struct Editor {
    current_doc: DocumentId,
    documents: HashMap<DocumentId, Document>,
    pub clipboard: Box<dyn ClipboardProvider>,
}

impl Editor {
    pub fn new() -> Self {
        let mut documents = HashMap::new();
        let mut scratch_doc = Document::scratch();
        let scratch_id = scratch_doc.id;
        documents.insert(scratch_id, scratch_doc);

        Editor {
            documents,
            current_doc: scratch_id,
            clipboard: clipboard::build_provider(),
        }
    }

    pub fn add_document(&mut self, doc: Document) {
        self.documents.insert(doc.id, doc);
    }

    pub fn current_document(&self) -> &Document {
        self.documents.get(&self.current_doc).unwrap()
    }

    pub fn current_document_mut(&mut self) -> &mut Document {
        self.documents.get_mut(&self.current_doc).unwrap()
    }

    pub fn switch_document(&mut self, doc_id: DocumentId) {
        self.current_doc = doc_id;
    }
}
