use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::Result;
use lsp_types::CompletionItem;
use noa_buffer::cursor::Position;
use noa_languages::{language::Language, lsp::Lsp};
use parking_lot::Mutex;
use tokio::{net::unix::OwnedWriteHalf, sync::oneshot};

use crate::protocol::{Request, RequestId, ToServer};

#[derive(PartialEq, Eq, Hash)]
enum ProxyKind {
    Lsp(&'static str /* language ID */),
}

pub struct Client {
    workspace_dir: PathBuf,
    daemon_connections: Arc<Mutex<HashMap<ProxyKind, OwnedWriteHalf>>>,
    sent_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<String>>>>,
    next_request_id: AtomicUsize,
}

impl Client {
    pub fn new(workspace_dir: &Path) -> Client {
        Client {
            workspace_dir: workspace_dir.to_owned(),
            daemon_connections: Arc::new(Mutex::new(HashMap::new())),
            sent_requests: Arc::new(Mutex::new(HashMap::new())),
            next_request_id: AtomicUsize::new(1),
        }
    }

    pub async fn completion(
        &self,
        lang: &Language,
        path: &Path,
        pos: Position,
    ) -> Result<Vec<CompletionItem>, ()> {
        todo!()
    }

    async fn send_request(&mut self, lang: &Lsp, request: Request) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let (tx, rx) = oneshot::channel::<String>();
        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst).into();
        self.sent_requests.lock().insert(id, tx);

        // Marshal a request.
        let mut body = serde_json::to_string(&ToServer::Request { id, body: request })?;
        body.push('\n');

        let proxy_kind = ProxyKind::Lsp(lang.language_id);
        for _ in 0..2 {
            self.spawn_proxy_if_needed(&proxy_kind).await?;

            // Send the request.
            match self
                .daemon_connections
                .lock()
                .get_mut(&proxy_kind)
                .unwrap()
                .write_all(body.as_bytes())
                .await
            {
                Ok(()) => {
                    break;
                }
                Err(err)
                    if err.kind() == ErrorKind::BrokenPipe
                        || err.kind() == ErrorKind::ConnectionRefused =>
                {
                    // Perhaps the LSP server has been exited due to the idle timeout.
                    // Try again.
                    trace!("sync is not available, respawning...");
                    self.daemon_connections.lock().remove(&proxy_kind);
                    continue;
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        todo!()
    }

    async fn spawn_proxy_if_needed(&mut self, kind: &ProxyKind) -> Result<()> {
        todo!()
    }
}

trait IntoLspPosition {
    fn into_lsp_position(self) -> lsp_types::Position;
}

impl IntoLspPosition for Position {
    fn into_lsp_position(self) -> lsp_types::Position {
        lsp_types::Position {
            line: self.y as u32,
            character: self.x as u32,
        }
    }
}
