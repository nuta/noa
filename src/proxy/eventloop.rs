use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use noa_proxy::protocol::ToServer;
use parking_lot::Mutex;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{UnixListener, UnixStream},
    time::timeout,
};

/// If the server does not receive any requests from clients for this duration,
/// the server will automatically exits.
const IDLE_STATE_MAX_SECS: u64 = 360;

pub struct EventLoop {
    sock_path: PathBuf,
    progress: Arc<Mutex<bool>>,
}

impl EventLoop {
    pub fn new(sock_path: &Path) -> EventLoop {
        EventLoop {
            sock_path: sock_path.to_owned(),
            progress: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn run(self) {
        let listener = match UnixListener::bind(&self.sock_path) {
            Ok(listener) => listener,
            Err(err) => {
                error!("Failed to bind to socket: {}", err);
                return;
            }
        };

        loop {
            match timeout(Duration::from_secs(IDLE_STATE_MAX_SECS), listener.accept()).await {
                Err(_) => {
                    // Timed out.
                    if !*self.progress.lock() {
                        info!("still in the idle state for a long while, exiting...");
                        return;
                    }

                    // If the server is not idle, progress will be set to true
                    // in next IDLE_STATE_MAX_SECS seconds.
                    *self.progress.lock() = false;
                }
                Ok(Ok((new_client, _))) => {
                    self.handle_client(new_client);
                }
                _ => {}
            }
        }
    }

    /// Spawns a new task to handle a client.
    pub fn handle_client(&self, client: UnixStream) {
        let progress = self.progress.clone();
        let (read_end, write_end) = client.into_split();
        tokio::spawn(async move {
            let mut reader = BufReader::new(read_end);
            let mut buf = String::with_capacity(128 * 1024);
            loop {
                buf.clear();

                // Receive a request from noa editor.
                match reader.read_line(&mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        *progress.lock() = true;

                        let m: ToServer = serde_json::from_str(&buf)
                            .with_context(|| format!("invalid request body: {}", buf))
                            .unwrap();
                    }
                    Err(err) => {
                        warn!("failed to read from a client: {}", err);
                        break;
                    }
                }
            }
        });
    }
}
