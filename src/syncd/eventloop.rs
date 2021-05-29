use std::{io::ErrorKind, path::Path, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use noa_common::warn_on_error;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixListener},
    spawn,
    sync::{mpsc::UnboundedReceiver, Mutex},
};

#[async_trait]
pub trait Daemon: Send {
    type Request: DeserializeOwned + Send;
    type Notification: Serialize + Send;
    async fn process_request(&mut self, request: Self::Request) -> Result<()>;
}

pub async fn eventloop<D: Daemon + 'static>(
    sock_path: &Path,
    mut daemon: D,
    mut noti_rx: UnboundedReceiver<D::Notification>,
) -> Result<()> {
    let listener = match UnixListener::bind(sock_path) {
        Ok(sock) => sock,
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            warn!("{} already exists", sock_path.display());
            return Err(err.into());
        }
        Err(err) => {
            return Err(err.into());
        }
    };

    let daemon_lock = Arc::new(Mutex::new(daemon));
    let clients = Arc::new(Mutex::new(Vec::<OwnedWriteHalf>::new()));

    {
        let clients = clients.clone();
        spawn(async move {
            while let Some(noti) = noti_rx.recv().await {
                for client in clients.lock().await.iter_mut() {
                    let json = serde_json::to_string(&noti).unwrap();
                    warn_on_error!(
                        client.write_all(json.as_bytes()).await,
                        "failed to write the response"
                    );
                }
            }
        });
    }

    loop {
        if let Ok((new_client, _)) = listener.accept().await {
            let (read_end, write_end) = new_client.into_split();
            let daemon_lock = daemon_lock.clone();
            let clients = clients.clone();
            spawn(async move {
                clients.lock().await.push(write_end);
                let mut reader = BufReader::new(read_end);
                let mut buf = String::with_capacity(16 * 1024);
                loop {
                    buf.clear();
                    match reader.read_line(&mut buf).await {
                        Ok(0) => break,
                        Ok(len) => {
                            let request: D::Request =
                                serde_json::from_str(&buf).expect("invalid request");
                            warn_on_error!(
                                daemon_lock.lock().await.process_request(request).await,
                                "error in process_request"
                            );
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

    Ok(())
}
