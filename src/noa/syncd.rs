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
    syncd_protocol::{LspRequest, LspResponse, Notification, Request, ToClient, ToServer},
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixStream},
    process::Command,
    sync::{oneshot, Mutex},
};

pub struct SyncdClient {
    workspace_dir: PathBuf,
    lsp_daemons: HashMap<&'static str /* lang id */, OwnedWriteHalf>,
    sent_requests: Arc<Mutex<HashMap<usize /* request id */, oneshot::Sender<LspResponse>>>>,
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
            next_request_id: 10000,
        }
    }

    pub async fn call_lsp_method<I: Serialize>(
        &mut self,
        lang: &'static Lang,
        request: I,
    ) -> Result<LspResponse> {
        use tokio::io::AsyncWriteExt;

        let (tx, rx) = oneshot::channel::<LspResponse>();
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
        Ok(rx.await?)
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

        // Handle responses from the server.
        let sent_requests = self.sent_requests.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(read_end);
            let mut buf = String::with_capacity(128 * 1024);
            loop {
                buf.clear();
                match reader.read_line(&mut buf).await {
                    Ok(0) => {
                        trace!("EOF returned from syncd");
                        break;
                    }
                    Ok(_) => {
                        let to_client: ToClient<LspResponse> = match serde_json::from_str(&buf) {
                            Ok(resp) => resp,
                            Err(err) => {
                                warn!("invalid packet from a syncd socket: {}", err);
                                break;
                            }
                        };

                        match to_client {
                            ToClient::Notification(noti) => {
                                // TODO:
                            }
                            ToClient::Response(resp) => {
                                match sent_requests.lock().await.remove(&resp.id) {
                                    Some(tx) => {
                                        tx.send(resp.body);
                                    }
                                    None => {
                                        warn!("unknown response id from syncd: {:#?}", resp);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        warn!("failed to read from a syncd socket: {}", err);
                        break;
                    }
                }
            }

            trace!("exiting syncd receive loop");
        });

        Ok(())
    }
}
