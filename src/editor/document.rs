use std::{
    collections::HashMap,
    fs::{create_dir_all, OpenOptions},
    io::ErrorKind,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::Result;

use arc_swap::ArcSwap;
use noa_buffer::buffer::Buffer;
use noa_common::{
    dirs::{backup_dir, noa_dir},
    oops::OopsExt,
    time_report::TimeReport,
};
use noa_languages::definitions::guess_language;
use noa_proxy::client::Client as ProxyClient;

use tokio::{sync::Notify, task::JoinHandle};

use crate::{
    completion::{Completion, CompletionItem, CompletionKind},
    flash::FlashManager,
    git::Repo,
    minimap::MiniMap,
    movement::{Movement, MovementState},
    view::View,
    words::Words,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentId(NonZeroUsize);

pub struct Document {
    id: DocumentId,
    version: usize,
    path: PathBuf,
    backup_path: Option<PathBuf>,
    name: String,
    buffer: Buffer,
    view: View,
    movement_state: MovementState,
    words: Words,
    completion: Arc<Completion>,
    flashes: FlashManager,
    minimap: Arc<ArcSwap<MiniMap>>,
    find_query: String,
    post_update_job: Option<JoinHandle<()>>,
}

impl Document {
    pub fn open_file(
        proxy: &Arc<ProxyClient>,
        id: DocumentId,
        path: PathBuf,
        name: String,
    ) -> Result<Document> {
        debug_assert!(path.is_absolute());

        let mut buffer = match OpenOptions::new().read(true).open(&path) {
            Ok(file) => Buffer::from_reader(file)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Buffer::new(),
            Err(err) => return Err(err.into()),
        };

        let backup_path = backup_dir().join(path.strip_prefix("/")?);
        if backup_path.exists() {
            notify_warn!("A backup file exists in {}", backup_dir().display());
        }

        let lang = guess_language(&path);
        buffer.set_language(lang);

        // Let the LSP server know about the file.
        {
            let raw_buffer = buffer.raw_buffer().clone();
            let proxy = proxy.clone();
            let path = path.clone();
            tokio::task::spawn_blocking(move || {
                let buffer_text = raw_buffer.text();
                tokio::spawn(async move {
                    proxy.open_file(lang, &path, &buffer_text).await.oops();
                });
            });
        }

        let words = Words::new_with_buffer(&buffer);
        Ok(Document {
            id,
            version: 1,
            path: path.to_owned(),
            backup_path: Some(backup_path),
            name,
            buffer,
            view: View::new(),
            movement_state: MovementState::new(),
            words,
            completion: Arc::new(Completion::new()),
            flashes: FlashManager::new(),
            minimap: Arc::new(ArcSwap::from_pointee(MiniMap::new())),
            find_query: String::new(),
            post_update_job: None,
        })
    }

    pub fn save_to_file(&mut self) -> Result<()> {
        self.buffer.save_undo();

        match self.buffer.save_to_file(&self.path) {
            Ok(()) => {
                if let Some(backup_path) = &self.backup_path {
                    std::fs::remove_file(backup_path).oops();
                }
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                trace!("saving {} with sudo", self.path.display());
                self.buffer.save_to_file_with_sudo(&self.path)?;
            }
            Err(err) => {
                return Err(anyhow::anyhow!("failed to save: {}", err));
            }
        }

        notify_info!("written {} lines", self.buffer.num_lines());
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_find_query<T: Into<String>>(&mut self, find_query: T) {
        self.find_query = find_query.into();
    }

    pub fn path(&self) -> &Path {
        &self.path
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

    pub fn flashes(&self) -> &FlashManager {
        &self.flashes
    }

    pub fn flashes_mut(&mut self) -> &mut FlashManager {
        &mut self.flashes
    }

    pub fn completion(&self) -> &Completion {
        &self.completion
    }

    pub fn minimap(&self) -> arc_swap::Guard<Arc<MiniMap>> {
        self.minimap.load()
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
        self.buffer
            .highlight(self.view.visible_range(), |range, span| {
                // trace!("syntax highlight: {:?} {:?}", range, span);
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

    pub fn post_update_job(
        &mut self,
        repo: &Option<Arc<Repo>>,
        proxy: &Arc<ProxyClient>,
        render_request: &Arc<Notify>,
    ) {
        let _time = TimeReport::new("post_update_jobs time");
        self.version += 1;

        // TODO:
        let changes = self.buffer.clear_changes();
        let updated_lines = 0..self.buffer.num_lines();

        self.buffer.post_update_hook();
        self.completion.clear();

        // Word completion.
        self.words.update_lines(&self.buffer, updated_lines);
        if let Some(current_word) = self.buffer.current_word_str() {
            self.words.fuzzyvec().filter_by_key(&current_word);
            for (word, _) in self.words.fuzzyvec().top_entries() {
                self.completion.push(CompletionItem {
                    kind: CompletionKind::CurrentWord,
                    insert_text: word,
                });
            }
        }

        if let Some(post_update_job) = self.post_update_job.take() {
            // Abort the previous run if it's still running.
            // TODO: I'm not sure if it works...
            post_update_job.abort();
        }

        let minimap = self.minimap.clone();
        let render_request = render_request.clone();
        let raw_buffer = self.buffer.raw_buffer().clone();

        let proxy = proxy.clone();
        let lang = self.buffer.language();
        let path = self.path.clone();
        let version = self.version;
        let repo = repo.clone();
        self.post_update_job = Some(tokio::task::spawn_blocking(move || {
            let _time = TimeReport::new("backgroung_post_update_jobs time");

            // This may take a time.
            let buffer_text = Arc::new(raw_buffer.text());

            // Synchronize the latest buffer text with the LSP server.
            {
                let buffer_text = buffer_text.clone();
                let path = path.clone();
                tokio::spawn(async move {
                    proxy
                        .update_file(lang, &path, &buffer_text, version)
                        .await
                        .oops();
                });
            }

            if let Some(repo) = repo {
                let mut new_minimap = MiniMap::new();
                new_minimap.update_git_line_statuses(&repo, &path, &buffer_text);
                minimap.store(Arc::new(new_minimap));
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
    pub fn new(proxy: &Arc<ProxyClient>) -> DocumentManager {
        let scratch_doc_id = DocumentId(
            // Safety: Obviously 1 is not zero.
            unsafe { NonZeroUsize::new_unchecked(1) },
        );
        let mut manager = DocumentManager {
            next_document_id: AtomicUsize::new(2),
            current: scratch_doc_id,
            documents: HashMap::new(),
        };

        let scratch_doc = Document::open_file(
            proxy,
            scratch_doc_id,
            noa_dir().join("scratch.txt"),
            "**scratch**".to_owned(),
        )
        .expect("failed to open scratch");
        manager.open(scratch_doc);
        manager
    }

    pub fn open_file(&mut self, proxy: &Arc<ProxyClient>, path: &Path) -> Result<&mut Document> {
        let doc_id = if let Some(doc) = self.get_document_by_path(path) {
            // Already opened. Just switch to it.
            let doc_id = doc.id;
            self.switch_current(doc_id);
            doc_id
        } else {
            // Allocate a document ID.
            let doc_id = DocumentId(
                NonZeroUsize::new(self.next_document_id.fetch_add(1, Ordering::SeqCst)).unwrap(),
            );

            let path = path.canonicalize()?;

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

            let doc = Document::open_file(proxy, doc_id, path, name)?;
            self.open(doc);
            doc_id
        };

        Ok(self.documents.get_mut(&doc_id).unwrap())
    }

    fn open(&mut self, doc: Document) {
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
        self.documents.values().find(|doc| doc.path == path)
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
}
