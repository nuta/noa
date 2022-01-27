use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::ErrorKind,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Result;

use noa_buffer::buffer::Buffer;
use noa_common::time_report::TimeReport;
use noa_languages::{definitions::PLAIN, language::Language};

use crate::{
    highlighting::Highlighter,
    movement::{Movement, MovementState},
    view::View,
    words::Words,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentId(NonZeroUsize);

pub struct Document {
    /// It's `None` if the document is not backed by a file (e.g. a scrach buffer).
    path: Option<PathBuf>,
    name: String,
    buffer: Buffer,
    lang: &'static Language,
    view: View,
    highlighter: Highlighter,
    movement_state: MovementState,
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
            view: View::new(),
            highlighter: Highlighter::new(lang),
            movement_state: MovementState::new(),
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
            match self.buffer.save_to_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                    trace!("saving {} with sudo", path.display());
                    self.buffer.save_to_file_with_sudo(path)?;
                }
                Err(err) => {
                    return Err(anyhow::anyhow!("failed to save: {}", err));
                }
            }
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

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    pub fn layout_view(&mut self, cols: usize) {
        self.view.layout(&self.buffer, cols);
    }

    pub fn move_cursors_up(&mut self) {
        self.movement().move_cursors_up();
    }

    pub fn move_cursors_down(&mut self) {
        self.movement().move_cursors_down();
    }

    pub fn move_cursors_left(&mut self) {
        self.movement().move_cursors_left();
    }

    pub fn move_cursors_right(&mut self) {
        self.movement().move_cursors_right();
    }

    pub fn select_up(&mut self) {
        self.movement().select_up();
    }

    pub fn select_down(&mut self) {
        self.movement().select_down();
    }

    pub fn select_left(&mut self) {
        self.movement().select_left();
    }

    pub fn select_right(&mut self) {
        self.movement().select_right();
    }

    pub fn select_until_beginning_of_line(&mut self) {
        self.movement().select_until_beginning_of_line();
    }

    pub fn select_until_end_of_line(&mut self) {
        self.movement().select_until_end_of_line();
    }

    pub fn add_cursors_up(&mut self) {
        todo!()
    }

    pub fn add_cursors_down(&mut self) {
        todo!()
    }

    pub fn delete_current_word(&mut self) {
        self.buffer.select_current_word();
        self.buffer.delete();
    }

    pub fn move_cursors_prev_word(&mut self) {
        self.buffer.move_to_prev_word();
    }

    pub fn move_cursors_next_word(&mut self) {
        self.buffer.move_to_next_word();
    }

    pub fn select_prev_word(&mut self) {
        self.buffer.select_prev_word();
    }

    pub fn select_next_word(&mut self) {
        self.buffer.select_prev_word();
    }

    pub fn post_update(&mut self) {
        let time = TimeReport::new("post_update_jobs time");

        // TODO:
        let updates_lines = 0..self.buffer.num_lines();

        self.words.update_lines(&self.buffer, updates_lines);
        self.highlighter.update(&self.buffer);
    }

    fn movement(&mut self) -> Movement<'_> {
        self.movement_state.movement(&mut self.buffer, &self.view)
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
        doc.post_update();

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
