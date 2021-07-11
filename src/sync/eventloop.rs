use std::{
    collections::HashMap,
    path::Path,
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use noa_common::{
    sync_protocol::{Notification, RawRequest, RawResponse, ToClient, ToServer},
    warn_on_error,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixListener},
    spawn,
    sync::{mpsc::UnboundedReceiver, Mutex},
    time::timeout,
};

const IDLE_STATE_MAX_SECS: u64 = 60 * 5;

#[async_trait]
pub trait Daemon: Send {
    type Request: DeserializeOwned + Send;
    type Response: Serialize + Send;
    async fn process_request(&mut self, request: Self::Request) -> Result<Self::Response>;
}

pub async fn eventloop<D: Daemon + 'static>(
    sock_path: &Path,
    daemon: D,
    mut noti_rx: UnboundedReceiver<Notification>,
) -> Result<()> {
    trace!("entering eventloop");

    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(sock_path).expect("failed to bind a unix domain socket");

    let daemon_lock = Arc::new(Mutex::new(daemon));
    let clients = Arc::new(Mutex::new(
        HashMap::<usize, Arc<Mutex<OwnedWriteHalf>>>::new(),
    ));

    // Broadcast notifications from the LSP server to all clients.
    {
        let clients = clients.clone();
        spawn(async move {
            while let Some(noti) = noti_rx.recv().await {
                trace!("sending a notification to noa: {:?}", noti);
                let mut json = serde_json::to_string(&ToClient::Notification(noti)).unwrap();

                json.push('\n');

                for client in clients.lock().await.values_mut() {
                    warn_on_error!(
                        client.lock().await.write_all(json.as_bytes()).await,
                        "failed to write the notification"
                    );
                }
            }
        });
    }

    // Handle requests from noa.
    let next_client_id = AtomicUsize::new(1);
    let progress = Arc::new(parking_lot::Mutex::new(false));
    loop {
        match timeout(Duration::from_secs(IDLE_STATE_MAX_SECS), listener.accept()).await {
            Err(_) => {
                // Timed out.
                if !*progress.lock() {
                    info!("idle state for a long while, exiting...");
                    return Ok(());
                }

                *progress.lock() = false;
            }
            Ok(Ok((new_client, _))) => {
                let client_id = next_client_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                info!("new client #{}", client_id);

                let (read_end, write_end) = new_client.into_split();
                let write_end = Arc::new(Mutex::new(write_end));
                let daemon_lock = daemon_lock.clone();
                let progress = progress.clone();
                let clients = clients.clone();
                spawn(async move {
                    clients.lock().await.insert(client_id, write_end.clone());
                    let mut reader = BufReader::new(read_end);
                    let mut buf = String::with_capacity(128 * 1024);
                    loop {
                        buf.clear();
                        // Receive a request from noa.
                        match reader.read_line(&mut buf).await {
                            Ok(0) => break, // EOF
                            Ok(_) => {
                                *progress.lock() = true;

                                let packet: ToServer<D::Request> = serde_json::from_str(&buf)
                                    .with_context(|| format!("invalid request body: {}", buf))
                                    .unwrap();
                                match packet {
                                    ToServer::Request(RawRequest { id, body: params }) => {
                                        match daemon_lock.lock().await.process_request(params).await
                                        {
                                            Ok(body) => {
                                                let resp = ToClient::Response(RawResponse {
                                                    id,
                                                    body: serde_json::to_string(&body).unwrap(),
                                                });
                                                let mut json =
                                                    serde_json::to_string(&resp).unwrap();

                                                json.push('\n');
                                                // Reply the response to noa.
                                                warn_on_error!(
                                                    write_end
                                                        .lock()
                                                        .await
                                                        .write_all(json.as_bytes())
                                                        .await,
                                                    "failed to write the response"
                                                );
                                            }
                                            Err(err) => {
                                                trace!("error in process_request: {}", err);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("failed to read from a client: {}", err);
                                break;
                            }
                        }
                    }

                    // The client has exited.
                    info!("client #{} is being closed", client_id);
                    clients.lock().await.remove(&client_id);
                });
            }
            _ => {}
        }
    }
}
