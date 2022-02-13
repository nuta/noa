use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{bail, Context, Result};
use lsp_types::Position;
use noa_common::{dirs::proxy_sock_path, oops::OopsExt};
use noa_languages::{language::Language, lsp::Lsp};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixStream},
    process::Command,
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    time::sleep,
};

use crate::protocol::{results, Notification, Request, RequestId, Response, ToClient, ToServer};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ProxyKind {
    Lsp(&'static str /* language ID */),
}

pub struct Client {
    workspace_dir: PathBuf,
    txs: Arc<Mutex<HashMap<ProxyKind, UnboundedSender<ToServer>>>>,
    sent_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<Response>>>>,
    notification_tx: UnboundedSender<Notification>,
    notification_rx: UnboundedReceiver<Notification>,
    next_request_id: AtomicUsize,
}

impl Client {
    pub fn new(workspace_dir: &Path) -> Client {
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();
        Client {
            workspace_dir: workspace_dir.to_owned(),
            txs: Arc::new(Mutex::new(HashMap::new())),
            sent_requests: Arc::new(Mutex::new(HashMap::new())),
            notification_tx,
            notification_rx,
            next_request_id: AtomicUsize::new(1),
        }
    }

    pub fn notification_rx(&self) -> &UnboundedReceiver<Notification> {
        &self.notification_rx
    }

    pub async fn completion(
        &self,
        lang: &Language,
        path: &Path,
        pos: (u32, u32),
    ) -> Result<results::Completion> {
        self.request(
            lang,
            Request::Completion {
                path: path.to_owned(),
                position: Position {
                    line: pos.0,
                    character: pos.1,
                },
            },
        )
        .await
    }

    pub async fn update_file(
        &self,
        lang: &Language,
        path: &Path,
        text: &str,
    ) -> Result<results::NoContent> {
        self.request(
            lang,
            Request::UpdateFile {
                path: path.to_owned(),
                text: text.to_owned(),
            },
        )
        .await
    }

    async fn request<R: DeserializeOwned>(&self, lang: &Language, request: Request) -> Result<R> {
        if let Some(lsp) = lang.lsp.as_ref() {
            let resp = self.do_request(lsp, request).await?;
            match resp {
                Response::Ok { results } => {
                    let res: R =
                        serde_json::from_value(results).context("unexpected LSP respones")?;
                    Ok(res)
                }
                Response::Err { reason } => Err(anyhow::anyhow!("LSP error: {}", reason)),
            }
        } else {
            bail!("LSP unavailable for {}", lang.id);
        }
    }

    async fn do_request(&self, lsp: &Lsp, request: Request) -> Result<Response> {
        let kind = ProxyKind::Lsp(lsp.language_id);
        let (resp_tx, resp_rx) = oneshot::channel();
        {
            let id = self.next_request_id.fetch_add(1, Ordering::SeqCst).into();
            self.sent_requests.lock().insert(id, resp_tx);

            let message = ToServer::Request { id, body: request };
            let mut proxies = self.txs.lock();
            match proxies.get(&kind) {
                Some(tx) => {
                    tx.send(message)?;
                }
                None => {
                    // The proxy client task has not been started yet. Enqueue the
                    // request and spawn the task to handle it asynchronously.
                    let (tx, rx) = mpsc::unbounded_channel();
                    tx.send(message)?;
                    proxies.insert(kind.clone(), tx);

                    let workspace_dir = self.workspace_dir.clone();
                    let sent_requests = self.sent_requests.clone();
                    let notification_tx = self.notification_tx.clone();

                    tokio::spawn(async move {
                        proxy_client_task(workspace_dir, kind, sent_requests, notification_tx, rx)
                            .await
                            .oops()
                    });
                }
            }
        }

        // Wait for the response and return it.
        Ok(resp_rx.await?)
    }
}

/// Receives a message from `Client`, spawns a proxy process, and writes the
/// message into the UNIX socket.
async fn proxy_client_task(
    workspace_dir: PathBuf,
    kind: ProxyKind,
    sent_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<Response>>>>,
    notification_tx: UnboundedSender<Notification>,
    mut rx: UnboundedReceiver<ToServer>,
) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let mut cached_write_end = None;
    while let Some(message) = rx.recv().await {
        let mut body = serde_json::to_string(&message).unwrap();
        body.push('\n');

        for _ in 0..2 {
            let write_end = match cached_write_end.as_mut() {
                Some(write_end) => write_end,
                None => {
                    cached_write_end = Some(
                        spawn_proxy(&workspace_dir, kind, &sent_requests, &notification_tx).await?,
                    );
                    cached_write_end.as_mut().unwrap()
                }
            };

            // Send the request.
            match write_end.write_all(body.as_bytes()).await {
                Ok(()) => {
                    break;
                }
                Err(err)
                    if err.kind() == ErrorKind::BrokenPipe
                        || err.kind() == ErrorKind::ConnectionRefused =>
                {
                    // Perhaps the proxy has been exited due to the idle timeout.
                    // Try again.
                    trace!("proxy is not available, respawning...");
                    cached_write_end = None;
                    continue;
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }
    }

    Ok(())
}

// Spawns a proxy process.
async fn spawn_proxy(
    workspace_dir: &Path,
    kind: ProxyKind,
    sent_requests: &Arc<Mutex<HashMap<RequestId, oneshot::Sender<Response>>>>,
    notification_tx: &UnboundedSender<Notification>,
) -> Result<OwnedWriteHalf> {
    // Spawn a process.
    let sock_id = match kind {
        ProxyKind::Lsp(lang) => {
            format!("lsp-{}", lang)
        }
    };

    let sock_path = proxy_sock_path(&workspace_dir, &sock_id);

    trace!("connecting to proxy {}", sock_path.display());
    if UnixStream::connect(&sock_path).await.is_err() {
        // The proxy for the language is not running. Spawn it.
        trace!("spawning lsp proxy at {}", sock_path.display());

        let mut cmd = if cfg!(debug_assertions) {
            info!("spawning from cargo");
            let mut cmd = Command::new("cargo");
            cmd.args(&["run", "--bin", "noa-sync", "--"]);
            cmd
        } else {
            Command::new("noa-proxy")
        };

        match kind {
            ProxyKind::Lsp(lang) => {
                cmd.arg("--mode");
                cmd.arg(lang);
            }
        }

        // TODO: daemonize
        cmd.arg("--workspace-dir")
            .arg(&workspace_dir)
            .arg("--sock-path")
            .arg(&sock_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn a proxy")?;
    }

    trace!("try connecting to proxy {}", sock_path.display());
    let sock = try_to_connect(&sock_path).await?;
    trace!("connected to proxy {}", sock_path.display());

    let (read_end, write_end) = sock.into_split();

    // Handle responses from the server.
    let sent_requests = sent_requests.clone();
    let notification_tx = notification_tx.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(read_end);
        let mut buf = String::with_capacity(128 * 1024);
        loop {
            buf.clear();
            match reader.read_line(&mut buf).await {
                Ok(0) => {
                    trace!("EOF returned from sync");
                    break;
                }
                Ok(_) => {
                    let resp: ToClient = match serde_json::from_str(&buf) {
                        Ok(resp) => resp,
                        Err(err) => {
                            warn!("invalid packet from a sync socket: {}", err);
                            break;
                        }
                    };

                    match resp {
                        ToClient::Notification(noti) => {
                            notification_tx.send(noti).ok();
                        }
                        ToClient::Response { id, body } => match sent_requests.lock().remove(&id) {
                            Some(tx) => {
                                tx.send(body).ok();
                            }
                            None => {
                                warn!("unknown response id from proxy: id={:?}, {:#?}", id, body);
                                break;
                            }
                        },
                    }
                }
                Err(err) => {
                    warn!("failed to read from a proxy socket: {}", err);
                    break;
                }
            }
        }

        trace!("exiting sync receive loop");
    });

    Ok(write_end)
}

async fn try_to_connect(sock_path: &Path) -> Result<UnixStream> {
    let mut last_err = None;
    for i in 0..20 {
        match UnixStream::connect(sock_path).await {
            Ok(sock) => return Ok(sock),
            Err(err) => {
                last_err = Some(err);
            }
        }

        sleep(Duration::from_millis(30 * i)).await;
    }

    Err(last_err.unwrap().into())
}
