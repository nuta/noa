use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use noa_common::{
    dirs::lsp_sock_path,
    syncd_protocol::{LspResponse, Notification, Request, ToClient, ToServer},
};
use noa_langs::Lang;
use serde::Serialize;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixStream},
    process::Command,
    sync::{oneshot, Mutex},
    time::sleep,
};

pub struct SyncdClient {
    workspace_dir: PathBuf,
    lsp_daemons: HashMap<&'static str /* lang id */, OwnedWriteHalf>,
    sent_requests: Arc<Mutex<HashMap<usize /* request id */, oneshot::Sender<LspResponse>>>>,
    next_request_id: usize,
}

impl SyncdClient {
    pub fn new<F>(workspace_dir: &Path, _notification_callback: F) -> SyncdClient
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

        // Construct a request.
        let mut body = serde_json::to_string(&ToServer::Request(Request { id, body: request }))?;
        body.push('\n');

        // Send the request.
        self.ensure_lsp_server_is_spawned(lang).await?;
        self.lsp_daemons
            .get_mut(lang.id)
            .unwrap()
            .write_all(body.as_bytes())
            .await?;

        // Wait for the response.
        Ok(rx.await?)
    }

    async fn ensure_lsp_server_is_spawned(&mut self, lang: &'static Lang) -> Result<()> {
        if self.lsp_daemons.contains_key(lang.id) {
            return Ok(());
        }

        let sock_path = lsp_sock_path(&self.workspace_dir, lang.id);
        if !sock_path.exists() {
            // The syncd for the language is not running. Spawn it.
            trace!("spawning lsp syncd at {}", sock_path.display());
            Command::new("noa-syncd")
                .arg("--daemon-type")
                .arg("lsp")
                .arg("--lang")
                .arg(lang.id)
                .arg("--workspace-dir")
                .arg(&self.workspace_dir)
                .arg("--sock-path")
                .arg(&sock_path)
                .spawn()
                .context("failed to spawn noa-syncd")?;
        }

        let sock = try_to_connect(&sock_path).await?;
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
                        // TODO:
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
                            ToClient::Notification(_noti) => {
                                // TODO:
                            }
                            ToClient::Response(resp) => {
                                match sent_requests.lock().await.remove(&resp.id) {
                                    Some(tx) => {
                                        tx.send(resp.body).unwrap();
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

async fn try_to_connect(sock_path: &Path) -> Result<UnixStream> {
    let mut last_err = None;
    for i in 0..5 {
        match UnixStream::connect(sock_path).await {
            Ok(sock) => return Ok(sock),
            Err(err) => {
                last_err = Some(err);
            }
        }

        sleep(Duration::from_millis(50 * i)).await;
    }

    return Err(last_err.unwrap().into());
}
