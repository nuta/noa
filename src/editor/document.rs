use std::{
    collections::HashMap,
    fs::{create_dir_all, OpenOptions},
    io::ErrorKind,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::Result;

use noa_buffer::{buffer::Buffer, cursor::Range};
use noa_common::{dirs::backup_dir, oops::OopsExt, time_report::TimeReport};
use noa_languages::{
    definitions::{guess_language, PLAIN},
    language::Language,
};
use parking_lot::RwLock;
use tokio::{sync::Notify, task::JoinHandle};

use crate::{
    editor::Editor,
    flash::FlashManager,
    git::Repo,
    highlighting::Highlighter,
    minimap::MiniMap,
    movement::{Movement, MovementState},
    view::View,
    words::Words,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentId(NonZeroUsize);

pub struct Document {
    id: DocumentId,
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
    flashes: FlashManager,
    minimap: Arc<RwLock<MiniMap>>,
    post_update_job: Option<JoinHandle<()>>,
}

impl Document {
    pub fn new(id: DocumentId, name: &str) -> Document {
        let lang = &PLAIN;
        Document {
            id,
            path: None,
            backup_path: None,
            name: name.to_string(),
            buffer: Buffer::new(),
            lang,
            view: View::new(),
            highlighter: Highlighter::new(lang),
            movement_state: MovementState::new(),
            words: Words::new(),
            flashes: FlashManager::new(),
            minimap: Arc::new(RwLock::new(MiniMap::new())),
            post_update_job: None,
        }
    }

    pub fn open_file(id: DocumentId, path: &Path) -> Result<Document> {
        let buffer = match OpenOptions::new().read(true).open(path) {
            Ok(file) => Buffer::from_reader(file)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Buffer::new(),
            Err(err) => return Err(err.into()),
        };

        // TODO: Create parent directories.
        let abs_path = path.canonicalize()?;
        let backup_path = backup_dir().join(abs_path.strip_prefix("/")?);
        if backup_path.exists() {
            notify_warn!("A backup file exists in {}", backup_dir().display());
        }

        // TODO:
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_str()
            .unwrap();

        let lang = guess_language(&abs_path);
        let words = Words::new_with_buffer(&buffer);
        Ok(Document {
            id,
            path: Some(abs_path),
            backup_path: Some(backup_path),
            name: name.to_string(),
            buffer,
            lang,
            view: View::new(),
            highlighter: Highlighter::new(lang),
            movement_state: MovementState::new(),
            words,
            flashes: FlashManager::new(),
            minimap: Arc::new(RwLock::new(MiniMap::new())),
            post_update_job: None,
        })
    }

    pub fn save_to_file(&mut self) -> Result<()> {
        self.buffer.save_undo();

        if let Some(ref path) = self.path {
            match self.buffer.save_to_file(path) {
                Ok(()) => {
                    if let Some(backup_path) = &self.backup_path {
                        std::fs::remove_file(backup_path).oops();
                    }
                }
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

    pub fn flashes(&self) -> &FlashManager {
        &self.flashes
    }

    pub fn flashes_mut(&mut self) -> &mut FlashManager {
        &mut self.flashes
    }

    pub fn minimap(&self) -> &Arc<RwLock<MiniMap>> {
        &self.minimap
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

        self.highlighter.highlight(&mut self.view);
        self.flashes.highlight(&mut self.view);
    }

    pub fn post_update_job(&mut self, repo: &Option<Arc<Repo>>, render_request: &Arc<Notify>) {
        let time = TimeReport::new("post_update_jobs time");

        // TODO:
        let updated_lines = 0..self.buffer.num_lines();

        self.words.update_lines(&self.buffer, updated_lines);
        self.highlighter.update(&self.buffer);

        if let Some(post_update_job) = self.post_update_job.take() {
            // Abort the previous run if it's still running.
            // TODO: It's meaninguless unless we "await" in the closure below.
            post_update_job.abort();
        }

        let minimap = self.minimap.clone();
        let render_request = render_request.clone();
        let raw_buffer = self.buffer.raw_buffer().clone();
        let git_diff_ctx = match (&self.path, repo) {
            (Some(path), Some(repo)) => Some((path.to_owned(), repo.clone())),
            _ => None,
        };

        self.post_update_job = Some(tokio::spawn(async move {
            let time = TimeReport::new("backgroung_post_update_jobs time");

            // This may take a time.
            let buffer_text = raw_buffer.text();

            if let Some((path, repo)) = git_diff_ctx {
                minimap
                    .write()
                    .update_git_line_statuses(&repo, &path, &buffer_text);
            }

            render_request.notify_one();
        }));
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
        let scratch_doc_id = DocumentId(
            // Safety: Obviously 1 is not zero.
            unsafe { NonZeroUsize::new_unchecked(1) },
        );
        let mut manager = DocumentManager {
            next_document_id: AtomicUsize::new(2),
            current: scratch_doc_id,
            documents: HashMap::new(),
        };

        let scratch_doc = Document::new(scratch_doc_id, "**scratch**");
        manager.open(scratch_doc);
        manager
    }

    pub fn open_file(&mut self, path: &Path) -> Result<&mut Document> {
        let doc_id = if let Some(doc) = self.get_document_by_path(path) {
            let doc_id = doc.id;
            self.switch_current(doc_id);
            doc_id
        } else {
            // Allocate a document ID.
            let doc_id = DocumentId(
                NonZeroUsize::new(self.next_document_id.fetch_add(1, Ordering::SeqCst)).unwrap(),
            );

            let doc = Document::open_file(doc_id, path)?;
            self.open(doc);
            doc_id
        };

        Ok(self.documents.get_mut(&doc_id).unwrap())
    }

    fn open(&mut self, mut doc: Document) {
        // First run of syntax highlighting, etc.
        // FIXME:
        // doc.post_update_job(editor);

        let doc_id = doc.id;
        debug_assert!(!self.documents.contains_key(&doc_id));
        self.documents.insert(doc_id, doc);
        self.switch_current(doc_id);
    }

    /// Switches the current buffer.
    pub fn switch_current(&mut self, doc_id: DocumentId) {
        self.current = doc_id;
    }

    pub fn get_document_by_path(&self, path: &Path) -> Option<&Document> {
        self.documents
            .values()
            .find(|doc| doc.path == Some(path.to_owned()))
    }

    pub fn current(&self) -> &Document {
        self.documents.get(&self.current).unwrap()
    }

    pub fn current_mut(&mut self) -> &mut Document {
        self.documents.get_mut(&self.current).unwrap()
    }
}
