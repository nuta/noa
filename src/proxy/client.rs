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
use lsp_types::{CompletionItem, HoverContents, TextEdit};
use nix::{sys::signal, unistd::Pid};
use noa_common::{
    dirs::{log_file_path, proxy_pid_path, proxy_sock_path},
    oops::OopsExt,
};
use noa_languages::Lsp;
use parking_lot::Mutex;
use tokio::{
    fs::{self},
    io::{AsyncBufReadExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixStream},
    process::Command,
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    time::{sleep, timeout},
};

use crate::protocol::{
    LspRequest, LspResponse, Notification, RequestId, Response, ToClient, ToServer,
};

const LSP_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, PartialEq, Eq, Hash)]
enum ProxyKind {
    Lsp(String /* language ID */),
}

pub struct Client {
    workspace_dir: PathBuf,
    txs: Arc<Mutex<HashMap<ProxyKind, UnboundedSender<ToServer>>>>,
    sent_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<Response>>>>,
    notification_tx: UnboundedSender<Notification>,
    next_request_id: AtomicUsize,
}

impl Client {
    pub fn new(workspace_dir: &Path, notification_tx: UnboundedSender<Notification>) -> Client {
        Client {
            workspace_dir: workspace_dir.to_owned(),
            txs: Arc::new(Mutex::new(HashMap::new())),
            sent_requests: Arc::new(Mutex::new(HashMap::new())),
            notification_tx,
            next_request_id: AtomicUsize::new(1),
        }
    }

    pub async fn open_file(&self, lsp: &Lsp, path: &Path, text: &str) -> Result<()> {
        match self
            .request(
                lsp,
                LspRequest::OpenFile {
                    path: path.to_owned(),
                    text: text.to_owned(),
                },
            )
            .await
        {
            Ok(LspResponse::NoContent) => Ok(()),
            Ok(other) => bail!("unexpected response: {:?}", other),
            Err(err) => Err(err),
        }
    }

    pub async fn update_file(
        &self,
        lsp: &Lsp,
        path: &Path,
        text: &str,
        version: usize,
    ) -> Result<()> {
        match self
            .request(
                lsp,
                LspRequest::UpdateFile {
                    path: path.to_owned(),
                    text: text.to_owned(),
                    version,
                },
            )
            .await
        {
            Ok(LspResponse::NoContent) => Ok(()),
            Ok(other) => bail!("unexpected response: {:?}", other),
            Err(err) => Err(err),
        }
    }

    pub async fn incremental_update_file(
        &self,
        lsp: &Lsp,
        path: &Path,
        edits: Vec<TextEdit>,
        version: usize,
    ) -> Result<()> {
        match self
            .request(
                lsp,
                LspRequest::IncrementalUpdateFile {
                    path: path.to_owned(),
                    edits,
                    version,
                },
            )
            .await
        {
            Ok(LspResponse::NoContent) => Ok(()),
            Ok(other) => bail!("unexpected response: {:?}", other),
            Err(err) => Err(err),
        }
    }

    pub async fn hover(
        &self,
        lsp: &Lsp,
        path: &Path,
        position: lsp_types::Position,
    ) -> Result<Option<HoverContents>> {
        match self
            .request(
                lsp,
                LspRequest::Hover {
                    path: path.to_owned(),
                    position,
                },
            )
            .await
        {
            Ok(LspResponse::Hover(contents)) => Ok(contents),
            Ok(other) => bail!("unexpected response: {:?}", other),
            Err(err) => Err(err),
        }
    }

    pub async fn completion(
        &self,
        lsp: &Lsp,
        path: &Path,
        position: lsp_types::Position,
    ) -> Result<Vec<CompletionItem>> {
        match self
            .request(
                lsp,
                LspRequest::Completion {
                    path: path.to_owned(),
                    position,
                },
            )
            .await
        {
            Ok(LspResponse::Completion(completions)) => Ok(completions),
            Ok(other) => bail!("unexpected response: {:?}", other),
            Err(err) => Err(err),
        }
    }

    pub async fn format(
        &self,
        lsp: &Lsp,
        path: &Path,
        options: lsp_types::FormattingOptions,
    ) -> Result<Vec<TextEdit>> {
        match self
            .request(
                lsp,
                LspRequest::Format {
                    path: path.to_owned(),
                    options,
                },
            )
            .await
        {
            Ok(LspResponse::Edits(edits)) => Ok(edits),
            Ok(other) => bail!("unexpected response: {:?}", other),
            Err(err) => Err(err),
        }
    }

    async fn request(&self, lsp: &Lsp, request: LspRequest) -> Result<LspResponse> {
        match timeout(
            LSP_REQUEST_TIMEOUT,
            self.do_request(&lsp.identifier, request),
        )
        .await
        {
            Ok(resp) => match resp? {
                Response::Ok { body } => {
                    serde_json::from_value::<LspResponse>(body).context("unexpected LSP respones")
                }
                Response::Err { reason } => Err(anyhow::anyhow!("LSP error: {}", reason)),
            },
            Err(_) => {
                bail!("LSP request timed out");
            }
        }
    }

    async fn do_request(&self, lang_indent: &str, request: LspRequest) -> Result<Response> {
        let kind = ProxyKind::Lsp(lang_indent.to_owned());
        let (resp_tx, resp_rx) = oneshot::channel();
        {
            let id = self.next_request_id.fetch_add(1, Ordering::SeqCst).into();
            self.sent_requests.lock().insert(id, resp_tx);

            let body: serde_json::Value =
                serde_json::to_value(request).context("serialize request")?;
            let message = ToServer::Request { id, body };
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
                        proxy_client_task(
                            workspace_dir,
                            kind.clone(),
                            sent_requests,
                            notification_tx,
                            rx,
                        )
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
                        spawn_proxy(&workspace_dir, &kind, &sent_requests, &notification_tx)
                            .await?,
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
    kind: &ProxyKind,
    sent_requests: &Arc<Mutex<HashMap<RequestId, oneshot::Sender<Response>>>>,
    notification_tx: &UnboundedSender<Notification>,
) -> Result<OwnedWriteHalf> {
    // Spawn a process.
    let proxy_id = match kind {
        ProxyKind::Lsp(lang) => {
            format!("lsp-{}", lang)
        }
    };

    let sock_path = proxy_sock_path(workspace_dir, &proxy_id);

    trace!("connecting to proxy {}", sock_path.display());
    if UnixStream::connect(&sock_path).await.is_err() {
        // The proxy for the language is not running. Spawn it.
        trace!("spawning lsp proxy at {}", sock_path.display());

        // Kill the existing proxy if it exists.
        let pid_path = proxy_pid_path(workspace_dir, &proxy_id);
        if let Ok(pid) = fs::read_to_string(&pid_path).await {
            let pid = pid.parse::<i32>().unwrap();
            trace!("killing existing lsp proxy {}", pid);

            let _ = signal::kill(Pid::from_raw(pid), signal::Signal::SIGKILL);
        }

        // Force remove the PID and sock files so that we can spawn a new one.
        let _ = fs::remove_file(&pid_path).await;
        let _ = fs::remove_file(&sock_path).await;

        let mut cmd = if cfg!(debug_assertions) {
            info!("spawning from cargo");
            let mut cmd = Command::new("cargo");
            cmd.args(&["run", "--bin", "noa-proxy", "--"]);
            cmd.env("RUST_LOG", "trace");
            cmd
        } else {
            Command::new("noa-proxy")
        };

        match kind {
            ProxyKind::Lsp(lang) => {
                cmd.arg("--mode");
                cmd.arg("lsp");
                cmd.arg("--lsp-language-id");
                cmd.arg(lang);
            }
        }

        let stdout_log = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_file_path(&proxy_id))
            .with_context(|| format!("failed to open proxy log file for {}", proxy_id))?;
        let stderr_log = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_file_path(&proxy_id))
            .with_context(|| format!("failed to open proxy log file for {}", proxy_id))?;

        cmd.arg("--workspace-dir")
            .arg(&workspace_dir)
            .arg("--sock-path")
            .arg(&sock_path)
            .arg("--pid-path")
            .arg(&pid_path)
            // .arg("--daemonize") TODO:
            .stdin(Stdio::null())
            .stdout(stdout_log)
            .stderr(stderr_log)
            .spawn()
            .with_context(|| format!("failed to spawn a proxy for {}", proxy_id))?;
    }

    trace!("try connecting to proxy {}", sock_path.display());
    let sock = try_to_connect(&sock_path).await.with_context(|| {
        format!(
            "failed to connect to the proxy socket: {}",
            sock_path.display()
        )
    })?;
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
    for i in 0..30 {
        match UnixStream::connect(sock_path).await {
            Ok(sock) => return Ok(sock),
            Err(err) => {
                last_err = Some(err);
            }
        }

        sleep(Duration::from_millis(20 * i)).await;
    }

    Err(last_err.unwrap().into())
}
