use std::{
    collections::HashMap,
    ffi::OsStr,
    fmt::Debug,
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
use noa_buffer::Point;
use noa_common::{
    dirs::sync_sock_path,
    oops::OopsExt,
    sync_protocol::{
        lsp_types::{self, HoverContents, SignatureHelp},
        BufferSyncRequest, FileLocation, LspRequest, LspResponse, Notification, RawRequest,
        ToClient, ToServer,
    },
};
use parking_lot::RwLock;
use serde::Serialize;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixStream},
    process::Command,
    sync::{mpsc::UnboundedSender, oneshot, Mutex},
    time::sleep,
};

use crate::buffer_set::OpenedFile;

pub struct SyncClient {
    workspace_dir: PathBuf,
    daemon_connections: Arc<Mutex<HashMap<&'static str /* lang id */, OwnedWriteHalf>>>,
    sent_requests: Arc<Mutex<HashMap<usize /* request id */, oneshot::Sender<String>>>>,
    next_request_id: AtomicUsize,
    noti_tx: UnboundedSender<Notification>,
}

impl SyncClient {
    pub fn new(workspace_dir: &Path, noti_tx: UnboundedSender<Notification>) -> SyncClient {
        SyncClient {
            workspace_dir: workspace_dir.to_owned(),
            daemon_connections: Arc::new(Mutex::new(HashMap::new())),
            sent_requests: Arc::new(Mutex::new(HashMap::new())),
            next_request_id: AtomicUsize::new(10000),
            noti_tx,
        }
    }

    pub async fn call_buffer_open_file(&self, path: &Path) -> Result<()> {
        self.call(
            "buffer_sync",
            None,
            BufferSyncRequest::OpenFile {
                path: path.to_path_buf(),
            },
        )
        .await?;
        Ok(())
    }

    pub async fn call_buffer_open_file_in_other(
        &self,
        pane_id: &str,
        path: &Path,
        position: Option<Point>,
    ) -> Result<()> {
        self.call(
            "buffer_sync",
            None,
            BufferSyncRequest::OpenFileInOther {
                pane_id: pane_id.to_string(),
                position,
                path: path.to_path_buf(),
            },
        )
        .await?;
        Ok(())
    }

    pub async fn call_buffer_update_file(&self, path: &Path, text: String) -> Result<()> {
        self.call(
            "buffer_sync",
            None,
            BufferSyncRequest::UpdateFile {
                path: path.to_path_buf(),
                text,
            },
        )
        .await?;
        Ok(())
    }

    pub async fn call_goto_definition(
        &self,
        opened_file: &Arc<RwLock<OpenedFile>>,
        pos: Option<Point>,
    ) -> Result<oneshot::Receiver<Vec<FileLocation>>> {
        self.call_lsp_method_for_file(
            opened_file,
            |path, opened_file| LspRequest::GoToDefinition {
                path,
                position: pos.unwrap_or_else(|| opened_file.buffer.main_cursor_pos()),
            },
            |resp| match resp {
                LspResponse::GoToDefinition(locs) => Ok(locs),
                _ => {
                    bail!("unexpected goto_definition response: {:?}", resp);
                }
            },
        )
        .await
    }

    pub async fn _call_hover(
        &self,
        opened_file: &Arc<RwLock<OpenedFile>>,
        pos: Option<Point>,
    ) -> Result<oneshot::Receiver<Option<HoverContents>>> {
        self.call_lsp_method_for_file(
            opened_file,
            |path, opened_file| LspRequest::Hover {
                path,
                position: pos.unwrap_or_else(|| opened_file.buffer.main_cursor_pos()),
            },
            |resp| match resp {
                LspResponse::Hover(contents) => Ok(contents),
                _ => {
                    bail!("unexpected hover response: {:?}", resp);
                }
            },
        )
        .await
    }

    pub async fn call_signature_help(
        &self,
        opened_file: &Arc<RwLock<OpenedFile>>,
        pos: Option<Point>,
    ) -> Result<oneshot::Receiver<Option<SignatureHelp>>> {
        self.call_lsp_method_for_file(
            opened_file,
            |path, opened_file| LspRequest::SignatureHelp {
                path,
                position: pos.unwrap_or_else(|| opened_file.buffer.main_cursor_pos()),
            },
            |resp| match resp {
                LspResponse::SignatureHelp(help) => Ok(help),
                _ => {
                    bail!("unexpected signature_help response: {:?}", resp);
                }
            },
        )
        .await
    }

    pub async fn call_completion(
        &self,
        opened_file: &Arc<RwLock<OpenedFile>>,
    ) -> Result<oneshot::Receiver<Vec<lsp_types::CompletionItem>>> {
        self.call_lsp_method_for_file(
            &opened_file,
            |path, opened_file| LspRequest::Completion {
                path,
                position: opened_file.buffer.main_cursor_pos(),
            },
            |resp| match resp {
                LspResponse::Completion(items) => Ok(items),
                _ => {
                    error!("unexpected completion response: {:?}", resp);
                    bail!("unexpected completion response: {:?}", resp);
                }
            },
        )
        .await
    }

    pub async fn call_lsp_method_for_file<F, G, I: Serialize, R1, R2>(
        &self,
        opened_file: &Arc<RwLock<OpenedFile>>,
        build_req: F,
        parse_resp: G,
    ) -> Result<oneshot::Receiver<R2>>
    where
        F: FnOnce(PathBuf, &OpenedFile) -> I,
        G: FnOnce(R1) -> Result<R2> + Send + 'static,
        R1: serde::de::DeserializeOwned,
        R2: Debug + Send + 'static,
    {
        let (result_tx, result_rx) = oneshot::channel();

        let (lang_id, req) = {
            let opened_file = opened_file.read();
            match (
                opened_file.buffer.lang().lsp.as_ref(),
                opened_file.buffer.path_for_lsp(&self.workspace_dir),
            ) {
                (Some(lsp), Some(path)) => (lsp.language_id, build_req(path, &*opened_file)),
                (Some(_), None) => bail!("not in workspace dir"),
                (None, _) => bail!("lsp not supported"),
            }
        };

        let resp_rx = self.call("lsp", Some(lang_id), req).await?;
        tokio::spawn(async move {
            let resp_body = match resp_rx.await {
                Ok(body) => body,
                Err(err) => {
                    error!("failed to receive response from sync server: {:?}", err);
                    return;
                }
            };

            let resp_raw: R1 = match serde_json::from_str(&resp_body) {
                Ok(resp) => resp,
                Err(err) => {
                    error!("failed to deserialize response from sync server: {:?}", err);
                    return;
                }
            };
            let resp: R2 = match parse_resp(resp_raw) {
                Ok(resp) => resp,
                Err(err) => {
                    error!("failed to parse response from sync server: {:?}", err);
                    return;
                }
            };

            result_tx.send(resp).oops();
        });

        Ok(result_rx)
    }

    pub async fn call<I: Serialize>(
        &self,
        daemon_type: &'static str,
        lsp_lang: Option<&'static str>,
        request: I,
    ) -> Result<oneshot::Receiver<String>> {
        use tokio::io::AsyncWriteExt;

        let (tx, rx) = oneshot::channel::<String>();
        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        self.sent_requests.lock().await.insert(id, tx);

        // Construct a request.
        let mut body = serde_json::to_string(&ToServer::Request(RawRequest { id, body: request }))?;
        body.push('\n');

        for _ in 0..2 {
            self.ensure_daemon_is_spawned(daemon_type, lsp_lang).await?;

            // Send the request.
            match self
                .daemon_connections
                .lock()
                .await
                .get_mut(daemon_type)
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
                    self.daemon_connections.lock().await.remove(daemon_type);
                    continue;
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        Ok(rx)
    }

    async fn ensure_daemon_is_spawned(
        &self,
        daemon_type: &'static str,
        lsp_lang: Option<&'static str>,
    ) -> Result<()> {
        static SPAWN_LOCK: Mutex<()> = Mutex::const_new(());
        let _spawn_lock = SPAWN_LOCK.lock().await;

        if self
            .daemon_connections
            .lock()
            .await
            .contains_key(daemon_type)
        {
            return Ok(());
        }

        let sock_path = sync_sock_path(&self.workspace_dir, daemon_type, lsp_lang);
        trace!("connecting to sync {}", sock_path.display());
        if UnixStream::connect(&sock_path).await.is_err() {
            // The sync for the language is not running. Spawn it.
            trace!("spawning lsp sync at {}", sock_path.display());

            let mut extra_args = Vec::new();
            if let Some(lang) = lsp_lang {
                extra_args.push("--lsp-lang");
                extra_args.push(lang);
            }

            spawn_sync(daemon_type, &self.workspace_dir, &sock_path, &extra_args)?;
        }

        trace!("try connecting to sync {}", sock_path.display());
        let sock = try_to_connect(&sock_path).await?;
        trace!("connected to sync {}", sock_path.display());

        let (read_end, write_end) = sock.into_split();
        self.daemon_connections
            .lock()
            .await
            .insert(daemon_type, write_end);

        // Handle responses from the server.
        let sent_requests = self.sent_requests.clone();
        let event_tx = self.noti_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(read_end);
            let mut buf = String::with_capacity(128 * 1024);
            loop {
                buf.clear();
                match reader.read_line(&mut buf).await {
                    Ok(0) => {
                        // TODO:
                        trace!("EOF returned from sync");
                        break;
                    }
                    Ok(_) => {
                        let to_client: ToClient = match serde_json::from_str(&buf) {
                            Ok(resp) => resp,
                            Err(err) => {
                                warn!("invalid packet from a sync socket: {}", err);
                                break;
                            }
                        };

                        match to_client {
                            ToClient::Notification(noti) => {
                                event_tx.send(noti).ok();
                            }
                            ToClient::Response(resp) => {
                                match sent_requests.lock().await.remove(&resp.id) {
                                    Some(tx) => {
                                        tx.send(resp.body).ok();
                                    }
                                    None => {
                                        warn!("unknown response id from sync: {:#?}", resp);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        warn!("failed to read from a sync socket: {}", err);
                        break;
                    }
                }
            }

            trace!("exiting sync receive loop");
        });

        Ok(())
    }
}

fn spawn_sync<A: AsRef<OsStr>>(
    daemon_type: &str,
    workspace_dir: &Path,
    sock_path: &Path,
    extra_args: &[A],
) -> Result<()> {
    if cfg!(debug_assertions) {
        info!("spawning from cargo");
        Command::new("cargo")
            .args(&["run", "--bin", "noa-sync", "--"])
            .arg("--daemon-type")
            .arg(daemon_type)
            .arg("--workspace-dir")
            .arg(&workspace_dir)
            .arg("--sock-path")
            .arg(&sock_path)
            .args(extra_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn noa-sync from cargo")?;
    } else {
        Command::new("noa-sync")
            .arg("--daemon-type")
            .arg(daemon_type)
            .arg("--workspace-dir")
            .arg(&workspace_dir)
            .arg("--sock-path")
            .arg(&sock_path)
            .args(extra_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn noa-sync")?;
    }

    Ok(())
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
