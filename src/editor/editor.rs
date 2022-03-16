use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use arc_swap::ArcSwap;
use futures::Future;
use noa_buffer::{
    buffer::Buffer,
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
};
use noa_common::oops::OopsExt;
use noa_compositor::{line_edit::LineEdit, Compositor};
use noa_languages::language::Lsp;
use noa_proxy::{client::Client as ProxyClient, lsp_types::TextEdit, protocol::Notification};
use tokio::sync::{broadcast, mpsc::UnboundedSender, Notify};

use crate::{
    clipboard::{self, ClipboardProvider},
    document::{Document, DocumentId, DocumentManager, OnChangeData},
    event_listener::EventListener,
    git::Repo,
    hook::HookManager,
    job::JobManager,
    linemap::LineMap,
};

pub struct Editor {
    pub workspace_dir: PathBuf,
    pub documents: DocumentManager,
    pub jobs: JobManager,
    pub hooks: HookManager,
    pub clipboard: Box<dyn ClipboardProvider>,
    pub find_query: LineEdit,
    pub repo: Option<Arc<Repo>>,
    pub proxy: Arc<noa_proxy::client::Client>,
    pub render_request: Arc<Notify>,
}

impl Editor {
    pub fn new(
        workspace_dir: &Path,
        render_request: Arc<Notify>,
        notification_tx: UnboundedSender<Notification>,
    ) -> Editor {
        let repo = match Repo::open(workspace_dir) {
            Ok(repo) => Some(Arc::new(repo)),
            Err(err) => {
                notify_warn!("failed to open the git repository: {}", err);
                None
            }
        };

        let proxy = Arc::new(noa_proxy::client::Client::new(
            workspace_dir,
            notification_tx,
        ));

        Editor {
            workspace_dir: workspace_dir.to_path_buf(),
            documents: DocumentManager::new(),
            jobs: JobManager::new(),
            hooks: HookManager::new(),
            clipboard: clipboard::build_provider(),
            find_query: LineEdit::new(),
            repo,
            proxy,
            render_request,
        }
    }

    pub fn current_buffer_mut(&mut self) -> &mut Buffer {
        self.documents.current_mut().buffer_mut()
    }

    pub fn open_file(&mut self, path: &Path, cursor_pos: Option<Position>) -> Result<DocumentId> {
        let mut doc = Document::new(path)?;

        // First run of tree sitter parsering, etc.
        doc.post_update_job();

        if let Some(lsp) = doc.buffer().language().lsp.as_ref() {
            tokio::spawn(lsp_file_sync_task(
                doc.subscribe_onchange(),
                doc.id(),
                self.proxy.clone(),
                doc.raw_buffer().clone(),
                doc.path().to_owned(),
                lsp,
            ));
        }

        tokio::spawn(git_diff_task(
            doc.subscribe_onchange(),
            self.repo.clone(),
            doc.linemap().clone(),
            doc.path().to_owned(),
            self.render_request.clone(),
        ));

        // Watch changes on disk and reload it if changed.
        if let Some(listener) = doc.modified_listener().cloned() {
            let doc_id = doc.id();
            self.jobs
                .listen_in_mainloop(listener, move |editor, _compositor| {
                    let current_id = editor.documents.current().id();
                    let doc = match editor.documents.get_mut_document_by_id(doc_id) {
                        Some(doc) => doc,
                        None => {
                            warn!("document {:?} was closed", doc_id);
                            return;
                        }
                    };

                    match doc.reload() {
                        Ok(_) => {
                            if current_id == doc.id() {
                                notify_info!("reloaded from the disk");
                            }
                        }
                        Err(err) => {
                            warn!("failed to reload {}: {:?}", doc.path().display(), err);
                        }
                    }
                });
        }

        if let Some(pos) = cursor_pos {
            doc.buffer_mut().move_main_cursor_to_pos(pos);
            doc.flashes_mut().flash(Range::from_positions(pos, pos));
        }

        let id = doc.id();
        self.documents.add(doc);
        Ok(id)
    }
}

/// Synchronizes the latest buffer text with the LSP server.
async fn lsp_file_sync_task(
    mut rx: broadcast::Receiver<OnChangeData>,
    _doc_id: DocumentId,
    proxy: Arc<ProxyClient>,
    initial_buffer: RawBuffer,
    path: PathBuf,
    lsp: &'static Lsp,
) {
    proxy
        .open_file(lsp, &path, &initial_buffer.text())
        .await
        .oops();

    let path = path.clone();
    while let Ok(OnChangeData {
        version,
        mut changes,
        ..
    }) = rx.recv().await
    {
        let edits = changes
            .drain(..)
            .map(|change| TextEdit {
                range: change.range.into(),
                new_text: change.insert_text,
            })
            .collect();

        proxy
            .incremental_update_file(lsp, &path, edits, version)
            .await
            .oops();
    }
}

async fn git_diff_task(
    mut rx: broadcast::Receiver<OnChangeData>,
    repo: Option<Arc<Repo>>,
    linemap: Arc<ArcSwap<LineMap>>,
    path: PathBuf,
    render_request: Arc<Notify>,
) {
    while let Ok(OnChangeData { raw_buffer, .. }) = rx.recv().await {
        if let Some(repo) = &repo {
            let buffer_text = raw_buffer.text();
            let mut new_linemap = LineMap::new();
            new_linemap.update_git_line_statuses(repo, &path, &buffer_text);
            linemap.store(Arc::new(new_linemap));
            render_request.notify_one();
        }
    }
}
