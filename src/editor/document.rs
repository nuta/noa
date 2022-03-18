use std::{
    collections::{HashMap, HashSet},
    env::current_dir,
    fs::{create_dir_all, OpenOptions},
    io::ErrorKind,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use anyhow::Result;

use arc_swap::ArcSwap;
use futures::executor::block_on;
use fuzzy_matcher::FuzzyMatcher;
use noa_buffer::{
    buffer::Buffer,
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
    undoable_raw_buffer::Change,
};
use noa_common::{
    dirs::{backup_dir, noa_dir},
    oops::OopsExt,
    prioritized_vec::PrioritizedVec,
};
use noa_proxy::client::Client as ProxyClient;

use noa_editorconfig::EditorConfig;
use noa_languages::language::guess_language;
use noa_terminal::terminal::is_raw_mode_enabled;
use tokio::{sync::broadcast, time::timeout};

use crate::{
    completion::{build_fuzzy_matcher, CompletionItem},
    editor::Editor,
    event_listener::EventListener,
    file_watch::{watch_file, FileWatcher},
    flash::FlashManager,
    linemap::LineMap,
    movement::{Movement, MovementState},
    view::View,
};

#[derive(Clone, Debug)]
pub struct OnChangeData {
    pub version: usize,
    pub raw_buffer: RawBuffer,
    pub changes: Vec<Change>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentId(NonZeroUsize);

pub struct Document {
    id: DocumentId,
    version: usize,
    last_saved_at: Option<SystemTime>,
    path: PathBuf,
    path_in_str: String,
    backup_path: Option<PathBuf>,
    virtual_file: bool,
    name: String,
    buffer: Buffer,
    saved_buffer: RawBuffer,
    view: View,
    movement_state: MovementState,
    completion_items: Vec<CompletionItem>,
    flashes: FlashManager,
    linemap: Arc<ArcSwap<LineMap>>,
    onchange_broadcast_tx: broadcast::Sender<OnChangeData>,
    _onchange_broadcast_rx: broadcast::Receiver<OnChangeData>,
    _watcher: Option<FileWatcher>,
    modified_listener: Option<EventListener>,
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

        buffer.set_editorconfig(EditorConfig::resolve_or_guess(&path));

        // Check if a backup file exists.
        let backup_path = backup_dir().join(path.strip_prefix("/")?);
        if backup_path.exists() {
            notify_warn!("A backup file exists in {}", backup_dir().display());
        }

        if let Some(lang) = guess_language(&path) {
            buffer.set_language(lang);
        }

        // I occationally see failure on watching a file. I'm still not sure
        // why it may happen though.
        let (watcher, modified_listener) = match watch_file(&path) {
            Ok((watcher, listener)) => (Some(watcher), Some(listener)),
            Err(err) => {
                warn!("failed to watch file {}: {}", path.display(), err);
                (None, None)
            }
        };

        let (onchange_broadcast_tx, onchange_broadcast_rx) = broadcast::channel(8);
        Ok(Document {
            id,
            version: 1,
            last_saved_at: None,
            path: path.to_owned(),
            path_in_str: path.to_str().unwrap().to_owned(),
            backup_path: Some(backup_path),
            virtual_file: false,
            name,
            saved_buffer: buffer.raw_buffer().clone(),
            buffer,
            view: View::new(),
            movement_state: MovementState::new(),
            completion_items: Vec::new(),
            flashes: FlashManager::new(),
            linemap: Arc::new(ArcSwap::from_pointee(LineMap::new())),
            onchange_broadcast_tx,
            _onchange_broadcast_rx: onchange_broadcast_rx,
            _watcher: watcher,
            modified_listener,
        })
    }

    pub fn save_to_file(&mut self, proxy: Option<&Arc<ProxyClient>>) -> Result<()> {
        self.buffer.save_undo();

        // Format the document using LSP.
        if let Some(proxy) = proxy {
            if let Some(lsp) = self.buffer.language().lsp.as_ref() {
                trace!("format on save: {}", self.path.display());
                let format_future =
                    proxy.format(lsp, &self.path, (*self.buffer.editorconfig()).into());
                match block_on(timeout(Duration::from_secs(3), format_future)) {
                    Ok(Ok(edits)) => {
                        self.buffer
                            .apply_text_edits(edits.into_iter().map(Into::into).collect());
                    }
                    Ok(Err(err)) => {
                        notify_warn!("LSP formatting failed");
                        warn!("LSP formatting failed: {}", err);
                    }
                    Err(_) => {
                        notify_warn!("LSP formatting timed out");
                    }
                }
            }
        }

        trace!("saving into a file: {}", self.path.display());
        let with_sudo = match self.buffer.save_to_file(&self.path) {
            Ok(()) => {
                if let Some(backup_path) = &self.backup_path {
                    let _ = std::fs::remove_file(backup_path);
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

        // FIXME: By any chance, the file was modified by another process
        // between saving the file and updating the last saved time here.
        //
        // For now, we just ignore that case.
        self.last_saved_at = Some(std::fs::metadata(&self.path)?.modified()?);

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

    pub fn set_completion_items(&mut self, items: Vec<CompletionItem>) {
        self.completion_items = items;
    }

    pub fn clear_completion_items(&mut self) {
        self.completion_items.clear();
    }

    pub fn is_virtual_file(&self) -> bool {
        self.virtual_file
    }

    pub fn set_virtual_file(&mut self, virtual_file: bool) {
        self.virtual_file = virtual_file;
    }

    pub fn is_dirty(&self) -> bool {
        let a = self.buffer.raw_buffer();
        let b = &self.saved_buffer;

        a.len_chars() != b.len_chars() && a != b
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn path_in_str(&self) -> &str {
        &self.path_in_str
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

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    pub fn subscribe_onchange(&self) -> broadcast::Receiver<OnChangeData> {
        self.onchange_broadcast_tx.subscribe()
    }

    pub fn modified_listener(&self) -> Option<&EventListener> {
        self.modified_listener.as_ref()
    }

    pub fn flashes(&self) -> &FlashManager {
        &self.flashes
    }

    pub fn flashes_mut(&mut self) -> &mut FlashManager {
        &mut self.flashes
    }

    pub fn completion_items(&self) -> &[CompletionItem] {
        &self.completion_items
    }

    pub fn linemap(&self) -> &Arc<ArcSwap<LineMap>> {
        &self.linemap
    }

    pub fn movement(&mut self) -> Movement<'_> {
        self.movement_state
            .movement(&mut self.buffer, &mut self.view)
    }

    pub fn layout_view(&mut self, find_query: &str, height: usize, width: usize) {
        self.view.layout(&self.buffer, height, width);
        self.view.clear_highlights(height);

        let visible_range = self.view.visible_range();

        // FIXME: Deal with the borrow checker and stop using this temporary vec
        //        to avoid unnecessary memory copies.
        let mut highlights = Vec::new();
        self.buffer.highlight(visible_range, |range, span| {
            highlights.push((range, span.to_owned()));
        });

        for (range, span) in highlights {
            self.view.highlight(range, &span);
        }

        // Highlight find matches in visible rows.
        for range in self
            .buffer
            .find_iter(find_query, self.view.first_visible_position())
        {
            if range.front() > self.view.last_visible_position() {
                break;
            }

            self.view.highlight(range, "buffer.find_match");
        }

        // Highlight a matching bracket.
        let main_pos = self.buffer.main_cursor().moving_position();
        if let Some(range) = self.buffer.matching_bracket(main_pos) {
            trace!("matching bracket: {:?}", range);
            self.view.highlight(range, "buffer.matching_bracket");
        }

        self.flashes.highlight(&mut self.view);
    }

    pub fn reload(&mut self) -> Result<()> {
        if self.is_dirty() {
            return Ok(());
        }

        if let Some(last_saved_at) = self.last_saved_at.as_ref() {
            if *last_saved_at >= std::fs::metadata(&self.path)?.modified()? {
                // The file hasn't been modified or modified by us. Ignore it.
                return Ok(());
            }
        }

        let file = OpenOptions::new().read(true).open(&self.path)?;
        self.buffer.save_undo();
        self.buffer.set_from_reader(file)?;
        self.saved_buffer = self.buffer.raw_buffer().clone();
        self.last_saved_at = Some(std::fs::metadata(&self.path)?.modified()?);

        Ok(())
    }

    /// Called when the buffer is modified.
    pub fn post_update_job(&mut self) {
        self.version += 1;
        let changes = self.buffer.post_update_hook();
        self.completion_items.clear();

        let _ = self.onchange_broadcast_tx.send(OnChangeData {
            version: self.version,
            raw_buffer: self.buffer.raw_buffer().clone(),
            changes,
        });
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

    pub fn open_file(&mut self, path: &Path, cursor_pos: Option<Position>) -> Result<DocumentId> {
        let mut doc = Document::new(path)?;

        // First run of tree sitter parsering, etc.
        doc.post_update_job();

        // Needs switch?
        // editor.hooks.invoke(Hook::AfterOpen { id: doc.id() });

        if let Some(pos) = cursor_pos {
            doc.buffer_mut().move_main_cursor_to_pos(pos);
            doc.flashes_mut().flash(Range::from_positions(pos, pos));
        }

        let id = doc.id();
        self.add(doc);
        Ok(id)
    }

    fn add(&mut self, doc: Document) {
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

    pub fn current(&self) -> &Document {
        self.documents.get(&self.current).unwrap()
    }

    pub fn current_mut(&mut self) -> &mut Document {
        self.documents.get_mut(&self.current).unwrap()
    }

    pub fn save_all_on_drop(&mut self, enable: bool) {
        self.save_all_on_drop = enable;
    }

    pub fn words(&self) -> Words {
        let buffers: Vec<RawBuffer> = self
            .documents
            .values()
            .map(|doc| doc.raw_buffer().clone())
            .collect();

        Words(buffers)
    }
}

impl Drop for DocumentManager {
    fn drop(&mut self) {
        // Check if it's safe to use eprintln!() here. It should be safe because
        // Terminal is already droppped to restore the terminal state.
        debug_assert_eq!(is_raw_mode_enabled().unwrap(), true);

        if self.save_all_on_drop {
            let mut failed_any = false;
            let mut num_saved_files = 0;
            for doc in self.documents.values_mut() {
                if let Err(err) = doc.save_to_file(None) {
                    eprintln!("failed to save {}: {}", doc.path().display(), err);
                    failed_any = true;
                } else {
                    num_saved_files += 1;
                }
            }

            if !failed_any {
                eprintln!("successfully saved {} files", num_saved_files);
            }
        }
    }
}

pub struct Words(Vec<RawBuffer>);

impl Words {
    pub fn search(self, query: &str, max_num_results: usize) -> PrioritizedVec<i64, String> {
        use rayon::prelude::*;

        const MIN_WORD_LEN: usize = 8;
        const MAX_NUM_WORDS_PER_BUFFER: usize = 10000;
        let fuzzy_matcher = build_fuzzy_matcher();

        // Scan all buffers to extract words in parallel.
        self.0
            .into_par_iter()
            .fold(
                || PrioritizedVec::with_max_capacity(max_num_results),
                |mut words, buffer| {
                    let mut seen_words = HashSet::new();
                    let iter = buffer.word_iter_from_beginning_of_word(Position::new(0, 0));
                    for word in iter.take(MAX_NUM_WORDS_PER_BUFFER) {
                        let text = word.text();
                        if text.len() < MIN_WORD_LEN {
                            continue;
                        }

                        if let Some(score) = fuzzy_matcher.fuzzy_match(&text, query) {
                            if !seen_words.contains(&text) {
                                words.insert(score, text.clone());
                                seen_words.insert(text);
                            }
                        }
                    }

                    words
                },
            )
            .reduce(
                || PrioritizedVec::with_max_capacity(max_num_results),
                |mut all_words, words| {
                    all_words.extend(words);
                    all_words
                },
            )
    }
}

#[cfg(test)]
mod tests {
    use noa_languages::language::get_language_by_name;
    use tempfile::NamedTempFile;

    use super::*;

    fn create_documents(
        num_files: usize,
        num_lines: usize,
    ) -> (DocumentManager, Vec<NamedTempFile>) {
        let text = &(format!("{}\n", "int helloworld; ".repeat(5))).repeat(num_lines);

        let mut documents = DocumentManager::new();
        let mut dummy_files = Vec::new();
        for _ in 0..num_files {
            let dummy_file = tempfile::NamedTempFile::new().unwrap();
            let mut doc = Document::new(dummy_file.path()).unwrap();
            doc.buffer_mut().insert(text);
            doc.buffer_mut()
                .set_language(get_language_by_name("c").unwrap());
            documents.add(doc);
            dummy_files.push(dummy_file);
        }

        (documents, dummy_files)
    }

    #[bench]
    fn bench_words_10_lines(b: &mut test::Bencher) {
        let (documents, _dummy_files) = create_documents(1, 10);
        b.iter(|| documents.words());
    }

    #[bench]
    fn bench_words_1000_lines(b: &mut test::Bencher) {
        let (documents, _dummy_files) = create_documents(1, 1000);
        b.iter(|| documents.words());
    }

    #[bench]
    fn bench_words_4_files(b: &mut test::Bencher) {
        let (documents, _dummy_files) = create_documents(4, 1000);
        b.iter(|| documents.words());
    }

    #[bench]
    fn bench_words_16_files(b: &mut test::Bencher) {
        let (documents, _dummy_files) = create_documents(16, 1000);
        b.iter(|| documents.words());
    }
}
