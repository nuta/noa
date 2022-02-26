use std::{
    collections::HashMap,
    env::current_dir,
    fs::{create_dir_all, OpenOptions},
    io::ErrorKind,
    num::NonZeroUsize,
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::Result;

use arc_swap::ArcSwap;
use noa_buffer::{buffer::Buffer, raw_buffer::RawBuffer, undoable_raw_buffer::Change};
use noa_common::{
    dirs::{backup_dir, noa_dir},
    fuzzyvec::FuzzyVec,
    oops::OopsExt,
};
use noa_languages::language::guess_language;

use crate::{
    completion::Completion,
    flash::FlashManager,
    linemap::LineMap,
    movement::{Movement, MovementState},
    view::View,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentId(NonZeroUsize);

pub struct Document {
    id: DocumentId,
    version: usize,
    path: PathBuf,
    backup_path: Option<PathBuf>,
    virtual_file: bool,
    name: String,
    buffer: Buffer,
    saved_buffer: RawBuffer,
    view: View,
    movement_state: MovementState,
    completion: Arc<Completion>,
    flashes: FlashManager,
    linemap: Arc<ArcSwap<LineMap>>,
    find_query: String,
    post_update_hook: Option<Box<dyn FnMut(usize /* version */, &RawBuffer, Vec<Change>)>>,
}

static NEXT_DOCUMENT_ID: AtomicUsize = AtomicUsize::new(1);

impl Document {
    pub fn new(path: &Path) -> Result<Document> {
        // Allocate a document ID.
        let id =
            DocumentId(NonZeroUsize::new(NEXT_DOCUMENT_ID.fetch_add(1, Ordering::SeqCst)).unwrap());

        // Make the path absolute. This is important since some components assume
        // that the path is absolute (e.g. LSP's document URI).
        //
        // Here we prefer Path::join() over Path::canonicalize() because it fails
        // if the path does not exist.
        let path = current_dir()?.join(path);

        // "/path/to/../parent/file" -> "parent/file"
        let mut name = String::new();
        for comp in path
            .components()
            .rev()
            .take(2)
            .map(|c| c.as_os_str().to_str().unwrap())
        {
            if !name.is_empty() {
                name.insert(0, '/');
            }

            name.insert_str(0, comp);
        }

        // Read the file contents.
        let mut buffer = match OpenOptions::new().read(true).open(&path) {
            Ok(file) => Buffer::from_reader(file)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Buffer::new(),
            Err(err) => {
                return Err(err.into());
            }
        };

        // Check if a backup file exists.
        let backup_path = backup_dir().join(path.strip_prefix("/")?);
        if backup_path.exists() {
            notify_warn!("A backup file exists in {}", backup_dir().display());
        }

        if let Some(lang) = guess_language(&path) {
            buffer.set_language(lang);
        }

        Ok(Document {
            id,
            version: 1,
            path: path.to_owned(),
            backup_path: Some(backup_path),
            virtual_file: false,
            name,
            saved_buffer: buffer.raw_buffer().clone(),
            buffer,
            view: View::new(),
            movement_state: MovementState::new(),
            completion: Arc::new(Completion::new()),
            flashes: FlashManager::new(),
            linemap: Arc::new(ArcSwap::from_pointee(LineMap::new())),
            find_query: String::new(),
            post_update_hook: None,
        })
    }

    pub fn set_post_update_hook<F>(&mut self, post_update_hook: F)
    where
        F: FnMut(usize /* version */, &RawBuffer, Vec<Change>) + 'static,
    {
        self.post_update_hook = Some(Box::new(post_update_hook));
    }

    pub fn save_to_file(&mut self) -> Result<()> {
        self.buffer.save_undo();

        let with_sudo = match self.buffer.save_to_file(&self.path) {
            Ok(()) => {
                if let Some(backup_path) = &self.backup_path {
                    std::fs::remove_file(backup_path).oops();
                }

                false
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                trace!("saving {} with sudo", self.path.display());
                self.buffer.save_to_file_with_sudo(&self.path)?;
                true
            }
            Err(err) => {
                return Err(anyhow::anyhow!("failed to save: {}", err));
            }
        };

        self.saved_buffer = self.buffer.raw_buffer().clone();
        notify_info!(
            "written {} lines{}",
            self.buffer.num_lines(),
            if with_sudo { " w/ sudo" } else { "" }
        );
        Ok(())
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name<S: Into<String>>(&mut self, name: S) {
        self.name = name.into();
    }

    pub fn is_virtual_file(&self) -> bool {
        self.virtual_file
    }

    pub fn set_virtual_file(&mut self, virtual_file: bool) {
        self.virtual_file = virtual_file;
    }

    pub fn set_find_query<T: Into<String>>(&mut self, find_query: T) {
        self.find_query = find_query.into();
    }

    pub fn is_dirty(&self) -> bool {
        let a = self.buffer.raw_buffer();
        let b = &self.saved_buffer;

        a.len_chars() != b.len_chars() && a != b
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn raw_buffer(&self) -> &RawBuffer {
        self.buffer.raw_buffer()
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn flashes(&self) -> &FlashManager {
        &self.flashes
    }

    pub fn flashes_mut(&mut self) -> &mut FlashManager {
        &mut self.flashes
    }

    pub fn completion(&self) -> &Completion {
        &self.completion
    }

    pub fn linemap(&self) -> &Arc<ArcSwap<LineMap>> {
        &self.linemap
    }

    pub fn movement(&mut self) -> Movement<'_> {
        self.movement_state
            .movement(&mut self.buffer, &mut self.view)
    }

    pub fn layout_view(&mut self, height: usize, width: usize) {
        self.view.layout(&self.buffer, height, width);
        self.view.clear_highlights(height);

        // FIXME: Deal with the borrow checker and stop using this temporary vec
        //        to avoid unnecessary memory copies.
        let mut highlights = Vec::new();
        let range = self.view.visible_range();
        self.buffer.highlight(range, |range, span| {
            highlights.push((range, span.to_owned()));
        });

        for (range, span) in highlights {
            self.view.highlight(range, &span);
        }

        // Highlight find matches in visible rows.
        for range in self
            .buffer
            .find_iter(&self.find_query, self.view.first_visible_position())
        {
            if range.front() > self.view.last_visible_position() {
                break;
            }

            self.view.highlight(range, "buffer.find_match");
        }

        self.flashes.highlight(&mut self.view);
    }

    pub fn update_completion(&mut self) {
        // Word completion.
        // TODO:
        // if let Some(current_word) = self.buffer.current_word_str() {
        //     let words = self.words.load();
        //     words.filter_by_key(&current_word);
        //     for (word, _) in words.top_entries() {
        //         self.completion.push(CompletionItem {
        //             kind: CompletionKind::CurrentWord,
        //             insert_text: word,
        //         });
        //     }
        // }
    }

    pub fn post_update_job(&mut self) {
        self.version += 1;
        let changes = self.buffer.post_update_hook();
        self.completion.clear();

        if let Some(hook) = &mut self.post_update_hook {
            (*hook)(self.version, self.buffer.raw_buffer(), changes);
        }
    }

    pub fn idle_job(&mut self) {
        let modified = self.buffer.save_undo();
        if modified {
            if let Some(ref backup_path) = self.backup_path {
                if let Some(parent_dir) = backup_path.parent() {
                    create_dir_all(parent_dir).oops();
                }
                self.buffer.save_to_file(backup_path).oops();
            }
        }
    }
}

pub struct DocumentManager {
    current: DocumentId,
    documents: HashMap<DocumentId, Document>,
    save_all_on_drop: bool,
}

impl DocumentManager {
    pub fn new() -> DocumentManager {
        let mut scratch_doc =
            Document::new(&noa_dir().join("scratch.txt")).expect("failed to open scratch");
        scratch_doc.set_name("**scratch**");
        scratch_doc.set_virtual_file(true);

        let mut manager = DocumentManager {
            current: scratch_doc.id,
            documents: HashMap::new(),
            save_all_on_drop: false,
        };
        manager.add(scratch_doc);
        manager
    }

    pub fn add(&mut self, doc: Document) {
        let doc_id = doc.id;
        debug_assert!(!self.documents.contains_key(&doc_id));
        self.documents.insert(doc_id, doc);
    }

    /// Switches the current buffer.
    pub fn switch_by_id(&mut self, doc_id: DocumentId) {
        self.current = doc_id;
    }

    pub fn switch_by_path(&mut self, path: &Path) -> Option<()> {
        if let Some(doc) = self.get_document_by_path(path) {
            let id = doc.id();
            self.switch_by_id(id);
            return Some(());
        }

        None
    }

    pub fn get_document_by_path(&self, path: &Path) -> Option<&Document> {
        self.documents.values().find(|doc| doc.path == path)
    }

    pub fn get_mut_document_by_id(&mut self, doc_id: DocumentId) -> Option<&mut Document> {
        self.documents.get_mut(&doc_id)
    }

    pub fn documents(&self) -> &HashMap<DocumentId, Document> {
        &self.documents
    }

    pub fn documents_mut(&mut self) -> &mut HashMap<DocumentId, Document> {
        &mut self.documents
    }

    pub fn current(&self) -> &Document {
        self.documents.get(&self.current).unwrap()
    }

    pub fn current_mut(&mut self) -> &mut Document {
        self.documents.get_mut(&self.current).unwrap()
    }

    pub fn save_all_on_drop(&mut self, enable: bool) {
        self.save_all_on_drop = enable;
    }

    pub fn words(&self) -> FuzzyVec<()> {
        const WORDS_SCAN_DURATION_MAX: Duration = Duration::from_millis(100);

        let mut started_at = Instant::now();
        let mut words = FuzzyVec::new();
        for doc in self.documents.values() {
            let buffer = doc.buffer();
            if let Some(syntax) = buffer.syntax().as_ref() {
                syntax.words(|range| {
                    let word = buffer.substr(range);
                    words.insert(word, (), 0);
                    if started_at.elapsed() >= WORDS_SCAN_DURATION_MAX {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                });
            }
        }
        words
    }
}

impl Drop for DocumentManager {
    fn drop(&mut self) {
        if self.save_all_on_drop {
            let mut failed_any = false;
            let mut num_saved_files = 0;
            for doc in self.documents.values_mut() {
                if let Err(err) = doc.save_to_file() {
                    notify_warn!("failed to save {}: {}", doc.path().display(), err);
                    failed_any = true;
                    num_saved_files += 1;
                }
            }

            if !failed_any {
                notify_info!("successfully saved {} files", num_saved_files);
            }
        }
    }
}
