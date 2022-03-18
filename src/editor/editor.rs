use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;

use noa_buffer::{
    buffer::Buffer,
    cursor::{Position, Range},
};

use noa_compositor::line_edit::LineEdit;

use noa_proxy::protocol::Notification;
use tokio::sync::{mpsc::UnboundedSender, Notify};

use crate::{
    clipboard::{self, ClipboardProvider},
    document::{Document, DocumentId, DocumentManager},
    file_watch,
    git::{self, Repo},
    job::JobManager,
    lsp,
};

pub struct Editor {
    pub workspace_dir: PathBuf,
    pub documents: DocumentManager,
    pub jobs: JobManager,
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
        doc.post_update_job(&self.proxy, self.repo.as_ref(), &self.render_request);

        file_watch::after_open_hook(&mut self.jobs, &doc);

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
}
