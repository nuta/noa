use std::{path::Path, sync::Arc};

use noa_proxy::protocol::Notification;
use tokio::sync::{mpsc::UnboundedSender, Notify};

use crate::{
    clipboard::{self, ClipboardProvider},
    document::DocumentManager,
    git::Repo,
};

pub struct Editor {
    pub documents: DocumentManager,
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
            documents: DocumentManager::new(&proxy),
            clipboard: clipboard::build_provider().unwrap_or_else(clipboard::build_dummy_provider),
            repo,
            proxy,
            render_request,
        }
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
