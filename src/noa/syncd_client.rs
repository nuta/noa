use std::{
    collections::HashMap,
    ffi::OsStr,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Stdio,
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

        for _ in 0..2 {
            self.ensure_lsp_server_is_spawned(lang).await?;

            // Send the request.
            match self
                .lsp_daemons
                .get_mut(lang.id)
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
                    self.lsp_daemons.remove(lang.id);
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

    async fn ensure_lsp_server_is_spawned(&mut self, lang: &'static Lang) -> Result<()> {
        static SPAWN_LOCK: Mutex<()> = Mutex::const_new(());
        let _spawn_lock = SPAWN_LOCK.lock().await;

        if self.lsp_daemons.contains_key(lang.id) {
            return Ok(());
        }

        let sock_path = lsp_sock_path(&self.workspace_dir, lang.id);
        if UnixStream::connect(&sock_path).await.is_err() {
            // The syncd for the language is not running. Spawn it.
            trace!("spawning lsp syncd at {}", sock_path.display());
            spawn_syncd("lsp", &self.workspace_dir, &sock_path, &["--lang", lang.id])?;
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
