use std::collections::HashMap;

use crate::document::{Document, DocumentId};

pub struct Editor {
    current_doc: DocumentId,
    documents: HashMap<DocumentId, Document>,
}

impl Editor {
    pub fn new() -> Self {
        let mut documents = HashMap::new();
        let mut scratch_doc = Document::scratch();
        scratch_doc.insert("Hello World from scratch!\n-----------------------\n\nby noa authors\nabcdedfghijklmnopqrstuvwxyzabcdedfghijklmnopqrstuvwxyzabcdedfghijklmnopqrstuvwxyzabcdedfghijklmnopqrstuvwxyzabcdedfghijklmnopqrstuvwxyzabcdedfghijklmnopqrstuvwxyzabcdedfghijklmnopqrstuvwxyzabcdedfghijklmnopqrstuvwxyzabcdedfghijklmnopqrstuvwxyz\n");
        let scratch_id = scratch_doc.id;
        documents.insert(scratch_id, scratch_doc);

        Editor {
            documents,
            current_doc: scratch_id,
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

    pub fn switch_document(&mut self, doc: &Document) {
        self.current_doc = doc.id;
    }
}
