use std::{
    collections::HashMap,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use arc_swap::ArcSwap;
use futures::{future::BoxFuture, Future, Stream};
use noa_buffer::{
    buffer::Buffer,
    cursor::{Position, Range},
    raw_buffer::RawBuffer,
    undoable_raw_buffer::Change,
};
use noa_common::oops::OopsExt;
use noa_compositor::{line_edit::LineEdit, Compositor};
use noa_languages::language::Language;
use noa_proxy::{client::Client as ProxyClient, lsp_types::TextEdit, protocol::Notification};
use tokio::sync::{
    broadcast,
    mpsc::{self, UnboundedSender},
    Notify,
};

use crate::{
    clipboard::{self, ClipboardProvider},
    document::{Document, DocumentId, DocumentManager, OnChangeData},
    event_listener::EventListener,
    git::Repo,
    job::JobManager,
    linemap::LineMap,
};

pub struct Editor {
    pub documents: DocumentManager,
    pub jobs: JobManager,
    pub clipboard: Box<dyn ClipboardProvider>,
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
            documents: DocumentManager::new(),
            jobs: JobManager::new(),
            clipboard: clipboard::build_provider(),
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

        tokio::spawn(lsp_file_sync_task(
            doc.subscribe_onchange(),
            doc.id(),
            self.proxy.clone(),
            doc.raw_buffer().clone(),
            doc.path().to_owned(),
            doc.buffer().language(),
        ));

        tokio::spawn(git_diff_task(
            doc.subscribe_onchange(),
            self.repo.clone(),
            doc.linemap().clone(),
            doc.path().to_owned(),
            self.render_request.clone(),
        ));

        if let Some(pos) = cursor_pos {
            doc.buffer_mut().move_main_cursor_to_pos(pos);
            doc.flashes_mut().flash(Range::from_positions(pos, pos));
        }

        let id = doc.id();
        self.documents.add(doc);
        Ok(id)
    }

    pub fn handle_notification(&mut self, notification: Notification) {
        match notification {
            Notification::Diagnostics { diags, path } => {
                if path != self.documents.current().path() {
                    return;
                }

                if let Some(diag) = diags.first() {
                    notify_warn!("{}: {:?}", diag.range.start.line + 1, diag.message);
                }
            }
        }
    }
    pub fn listen_in_mainloop<Callback>(&mut self, listener: EventListener, callback: Callback)
    where
        Callback: FnMut(&mut Editor, &mut Compositor<Editor>) + Send + 'static,
    {
        self.jobs.push_event_listener(listener, callback);
    }

    pub fn await_in_mainloop<Fut, Ret, Then>(&mut self, future: Fut, then: Then)
    where
        Fut: Future<Output = Result<Ret>> + Send + 'static,
        Ret: Send + 'static,
        Then: FnOnce(&mut Editor, &mut Compositor<Editor>, Ret) + Send + 'static,
    {
        self.jobs.push_future(future, then);
    }
}

/// Synchronizes the latest buffer text with the LSP server.
async fn lsp_file_sync_task(
    mut rx: broadcast::Receiver<OnChangeData>,
    _doc_id: DocumentId,
    proxy: Arc<ProxyClient>,
    initial_buffer: RawBuffer,
    path: PathBuf,
    lang: &'static Language,
) {
    proxy
        .open_file(lang, &path, &initial_buffer.text())
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
            .incremental_update_file(lang, &path, edits, version)
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
