use std::collections::HashMap;

use crate::document::{Document, DocumentId};

pub struct Editor {
    documents: HashMap<DocumentId, Document>,
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            documents: HashMap::new(),
        }
    }

    pub fn add_document(&mut self, doc: Document) {
        self.documents.insert(doc.id, doc);
    }
}
