use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use noa_buffer::Lang;
use noa_common::{
    dirs::lsp_sock_path,
    syncd_protocol::{Notification, Request, ToServer},
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::AsyncWriteExt,
    net::{unix::OwnedWriteHalf, UnixStream},
    process::Command,
    sync::{oneshot, Mutex},
};

pub struct SyncdClient {
    workspace_dir: PathBuf,
    lsp_daemons: HashMap<&'static str /* lang id */, OwnedWriteHalf>,
    sent_requests: Arc<Mutex<HashMap<usize /* request id */, oneshot::Sender<String>>>>,
    next_request_id: usize,
}

impl SyncdClient {
    pub fn new<F>(workspace_dir: &Path, notification_callback: F) -> SyncdClient
    where
        F: FnMut(Notification),
    {
        SyncdClient {
            workspace_dir: workspace_dir.to_owned(),
            lsp_daemons: HashMap::new(),
            sent_requests: Arc::new(Mutex::new(HashMap::new())),
            next_request_id: 1,
        }
    }

    pub async fn lsp_request<I: Serialize, R: DeserializeOwned>(
        &mut self,
        lang: &'static Lang,
        request: I,
    ) -> Result<R> {
        use tokio::io::AsyncWriteExt;

        let (tx, rx) = oneshot::channel::<String>();
        let id = self.next_request_id;
        self.next_request_id += 1;
        self.sent_requests.lock().await.insert(id, tx);

        // Send the request.
        self.spawn_and_connect_lsp_server(lang).await?;

        let mut body = serde_json::to_string(&ToServer::Request(Request { id, body: request }))?;
        body.push('\n');

        self.lsp_daemons
            .get_mut(lang.id)
            .unwrap()
            .write_all(body.as_bytes())
            .await?;

        // Wait for the response.
        let ret = serde_json::from_str(&rx.await?)?;
        Ok(ret)
    }

    async fn spawn_and_connect_lsp_server(&mut self, lang: &'static Lang) -> Result<()> {
        let sock_path = lsp_sock_path(&self.workspace_dir, lang.id);
        if !sock_path.exists() {
            // The syncd for the language is not running. Spawn it.
            todo!("The syncd for the language is not running. Spawn it.")
        }

        let sock = UnixStream::connect(sock_path).await?;
        let (read_end, write_end) = sock.into_split();
        self.lsp_daemons.insert(lang.id, write_end);
        Ok(())
    }
}
