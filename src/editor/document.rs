use std::{
    collections::HashMap,
    fs::OpenOptions,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Result;

use noa_buffer::buffer::Buffer;
use noa_common::time_report::TimeReport;
use noa_languages::{definitions::PLAIN, language::Language};

use crate::{highlighting::Highlighter, view::View, words::Words};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentId(NonZeroUsize);

pub struct Document {
    /// It's `None` if the document is not backed by a file (e.g. a scrach buffer).
    path: Option<PathBuf>,
    name: String,
    buffer: Buffer,
    lang: &'static Language,
    view: View,
    words: Words,
}

impl Document {
    pub fn new(name: &str) -> Document {
        let lang = &PLAIN;
        Document {
            path: None,
            name: name.to_string(),
            buffer: Buffer::new(),
            lang,
            view: View::new(Highlighter::new(lang)),
            words: Words::new(),
        }
    }

    pub fn open_file(path: &Path) -> Result<Document> {
        let file = OpenOptions::new().read(true).create(true).open(path)?;
        let buffer = Buffer::from_reader(file)?;
        let words = Words::new_with_buffer(&buffer);
        unimplemented!()
    }

    pub fn save_to_file(&mut self) -> Result<()> {
        self.buffer.save_undo();

        if let Some(ref path) = self.path {
            self.buffer.save_to_file(path)?;
        }

        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn layout_view(&mut self, rows: usize, cols: usize) {
        self.view.layout(&self.buffer, rows, cols);
    }

    pub fn run_post_update_jobs(&mut self) {
        let time = TimeReport::new("post_update_jobs time");

        // TODO:
        let updates_lines = 0..self.buffer.num_lines();

        self.words.update_lines(&self.buffer, updates_lines);

        self.view.post_update(&self.buffer);
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
                // Safety: Obviously 1 is not zero. This is a dummy value and
                //         will be updated soon by a `open_virtual_file` call below.
                unsafe { NonZeroUsize::new_unchecked(1) },
            ),
            documents: HashMap::new(),
        };

        let scratch_doc = Document::new("**scratch**");
        manager.open_virtual_file(scratch_doc);
        manager
    }

    pub fn open_virtual_file(&mut self, mut doc: Document) {
        // Allocate a document ID.
        let doc_id = DocumentId(
            NonZeroUsize::new(self.next_document_id.fetch_add(1, Ordering::SeqCst)).unwrap(),
        );

        // First run of syntax highlighting, etc.
        doc.run_post_update_jobs();

        self.documents.insert(doc_id, doc);

        // Switch to the buffer.
        self.current = doc_id;
    }

    pub fn current(&self) -> &Document {
        self.documents.get(&self.current).unwrap()
    }

    pub fn current_mut(&mut self) -> &mut Document {
        self.documents.get_mut(&self.current).unwrap()
    }
}
