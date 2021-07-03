use std::{
    collections::HashMap,
    ffi::OsStr,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use noa_common::{
    dirs::lsp_sock_path,
    syncd_protocol::{
        lsp_types, BufferSyncRequest, FileLocation, LspRequest, LspResponse, Notification,
        RawRequest, ToClient, ToServer,
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

use crate::editor::OpenedFile;

pub struct SyncdClient {
    workspace_dir: PathBuf,
    daemon_connections: HashMap<&'static str /* lang id */, OwnedWriteHalf>,
    sent_requests: Arc<Mutex<HashMap<usize /* request id */, oneshot::Sender<String>>>>,
    next_request_id: usize,
    noti_tx: UnboundedSender<Notification>,
}

impl SyncdClient {
    pub fn new(workspace_dir: &Path, noti_tx: UnboundedSender<Notification>) -> SyncdClient {
        SyncdClient {
            workspace_dir: workspace_dir.to_owned(),
            daemon_connections: HashMap::new(),
            sent_requests: Arc::new(Mutex::new(HashMap::new())),
            next_request_id: 10000,
            noti_tx,
        }
    }

    pub async fn call_buffer_open_file(&mut self, path: &Path) -> Result<()> {
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

    pub async fn call_buffer_update_file(&mut self, path: &Path, text: String) -> Result<()> {
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
        &mut self,
        opened_file: &Arc<RwLock<OpenedFile>>,
    ) -> Result<Vec<FileLocation>> {
        let resp = self
            .call_lsp_method_for_file(opened_file, |path, opened_file| {
                LspRequest::GoToDefinition {
                    path,
                    position: opened_file.buffer.main_cursor_pos(),
                }
            })
            .await;

        match resp {
            Ok(LspResponse::GoToDefinition(locs)) => return Ok(locs),
            _ => {
                bail!("unexpected goto_definition response: {:?}", resp);
            }
        }
    }

    pub async fn call_completion(
        &mut self,
        opened_file: &Arc<RwLock<OpenedFile>>,
    ) -> Result<Vec<lsp_types::CompletionItem>> {
        let resp = self
            .call_lsp_method_for_file(opened_file, |path, opened_file| LspRequest::Completion {
                path,
                position: opened_file.buffer.main_cursor_pos(),
            })
            .await;

        match resp {
            Ok(LspResponse::Completion(items)) => return Ok(items),
            _ => {
                bail!("unexpected goto_definition response: {:?}", resp);
            }
        }
    }

    pub async fn call_lsp_method_for_file<F, I: Serialize>(
        &mut self,
        opened_file: &Arc<RwLock<OpenedFile>>,
        build_req: F,
    ) -> Result<LspResponse>
    where
        F: FnOnce(PathBuf, &OpenedFile) -> I,
    {
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

        let resp_body = self.call("lsp", Some(lang_id), req).await?;
        let resp = serde_json::from_str(&resp_body)?;
        Ok(resp)
    }

    pub async fn call<I: Serialize>(
        &mut self,
        daemon_type: &'static str,
        lang: Option<&'static str>,
        request: I,
    ) -> Result<String> {
        use tokio::io::AsyncWriteExt;

        let (tx, rx) = oneshot::channel::<String>();
        let id = self.next_request_id;
        self.next_request_id += 1;
        self.sent_requests.lock().await.insert(id, tx);

        // Construct a request.
        let mut body = serde_json::to_string(&ToServer::Request(RawRequest { id, body: request }))?;
        body.push('\n');

        for _ in 0..2 {
            self.ensure_daemon_is_spawned(daemon_type, lang).await?;

            // Send the request.
            match self
                .daemon_connections
                .get_mut(daemon_type)
                .unwrap()
                .write_all(body.as_bytes())
                .await
            {
                Ok(()) => break,
                Err(err)
                    if err.kind() == ErrorKind::BrokenPipe
                        || err.kind() == ErrorKind::ConnectionRefused =>
                {
                    // Perhaps the LSP server has been exited due to the idle timeout.
                    // Try again.
                    trace!("syncd is not available, respawning...");
                    self.daemon_connections.remove(daemon_type);
                    continue;
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        // Wait for the response.
        Ok(rx.await?)
    }

    async fn ensure_daemon_is_spawned(
        &mut self,
        daemon_type: &'static str,
        lang: Option<&'static str>,
    ) -> Result<()> {
        static SPAWN_LOCK: Mutex<()> = Mutex::const_new(());
        let _spawn_lock = SPAWN_LOCK.lock().await;

        if self.daemon_connections.contains_key(daemon_type) {
            return Ok(());
        }

        let sock_path = lsp_sock_path(&self.workspace_dir, daemon_type);
        trace!("connecting to syncd {}", sock_path.display());
        if UnixStream::connect(&sock_path).await.is_err() {
            // The syncd for the language is not running. Spawn it.
            trace!("spawning lsp syncd at {}", sock_path.display());

            let mut extra_args = Vec::new();
            if let Some(lang) = lang {
                extra_args.push("--lang");
                extra_args.push(lang);
            }

            spawn_syncd(daemon_type, &self.workspace_dir, &sock_path, &extra_args)?;
        }

        let sock = try_to_connect(&sock_path).await?;
        let (read_end, write_end) = sock.into_split();
        self.daemon_connections.insert(daemon_type, write_end);

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
                        trace!("EOF returned from syncd");
                        break;
                    }
                    Ok(_) => {
                        let to_client: ToClient<String> = match serde_json::from_str(&buf) {
                            Ok(resp) => resp,
                            Err(err) => {
                                warn!("invalid packet from a syncd socket: {}", err);
                                break;
                            }
                        };

                        match to_client {
                            ToClient::Notification(noti) => {
                                event_tx.send(noti).unwrap();
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

fn spawn_syncd<A: AsRef<OsStr>>(
    daemon_type: &str,
    workspace_dir: &Path,
    sock_path: &Path,
    extra_args: &[A],
) -> Result<()> {
    if cfg!(debug_assertions) {
        Command::new("cargo")
            .args(&["run", "--bin", "noa-syncd", "--"])
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
            .context("failed to spawn noa-syncd from cargo")?;
    } else {
        Command::new("noa-syncd")
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
            .context("failed to spawn noa-syncd")?;
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
