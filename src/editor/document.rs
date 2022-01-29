use std::{
    collections::HashMap,
    fs::{create_dir_all, OpenOptions},
    io::ErrorKind,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Result;

use noa_buffer::buffer::Buffer;
use noa_common::{dirs::backup_dir, oops::OopsExt, time_report::TimeReport};
use noa_languages::{
    definitions::{guess_language, PLAIN},
    language::Language,
};

use crate::{
    editor::Editor,
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
    backup_path: Option<PathBuf>,
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
            backup_path: None,
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
        let buffer = match OpenOptions::new().read(true).open(path) {
            Ok(file) => Buffer::from_reader(file)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Buffer::new(),
            Err(err) => return Err(err.into()),
        };

        // TODO: Create parent directories.
        let abs_path = path.canonicalize()?;
        let backup_path = backup_dir().join(abs_path.strip_prefix("/")?);

        // TODO:
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_str()
            .unwrap();

        let lang = guess_language(&abs_path);
        let words = Words::new_with_buffer(&buffer);
        Ok(Document {
            path: Some(path.to_owned()),
            backup_path: Some(backup_path),
            name: name.to_string(),
            buffer,
            lang,
            view: View::new(),
            highlighter: Highlighter::new(lang),
            movement_state: MovementState::new(),
            words,
        })
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

    pub fn movement(&mut self) -> Movement<'_> {
        self.movement_state
            .movement(&mut self.buffer, &mut self.view)
    }

    pub fn layout_view(&mut self, height: usize, width: usize) {
        // TODO:
        let updated_lines = 0..self.buffer.num_lines();

        self.view.layout(&self.buffer, height, width);
        self.view.clear_highlights(updated_lines);
        self.highlighter.update(&self.buffer);
        self.highlighter.highlight(&mut self.view);
    }

    pub fn post_update_job(&mut self) {
        let time = TimeReport::new("post_update_jobs time");

        // TODO:
        let updated_lines = 0..self.buffer.num_lines();

        self.words.update_lines(&self.buffer, updated_lines.clone());
    }

    pub fn idle_job(&mut self) {
        self.buffer.save_undo();

        if let Some(ref backup_path) = self.backup_path {
            if let Some(parent_dir) = backup_path.parent() {
                create_dir_all(parent_dir).oops();
            }
            self.buffer.save_to_file(backup_path).oops();
        }
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
        manager.open(scratch_doc);
        manager
    }

    pub fn open_file(&mut self, path: &Path) -> Result<()> {
        let doc = Document::open_file(path)?;
        self.open(doc);
        Ok(())
    }

    pub fn open(&mut self, mut doc: Document) {
        // Allocate a document ID.
        let doc_id = DocumentId(
            NonZeroUsize::new(self.next_document_id.fetch_add(1, Ordering::SeqCst)).unwrap(),
        );

        // First run of syntax highlighting, etc.
        doc.post_update_job();

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
