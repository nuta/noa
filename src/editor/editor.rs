use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use noa_proxy::protocol::Notification;
use tokio::sync::{mpsc::UnboundedSender, Notify};

use crate::{
    clipboard::{self, ClipboardProvider},
    document::DocumentManager,
    git::Repo,
    hook::HookManager,
    job::JobManager,
    ui::line_edit::LineEdit,
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
}
