use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Context;
use noa_common::oops::OopsExt;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixListener, UnixStream},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};

use crate::{
    protocol::{Notification, RequestId, Response, ToClient, ToServer},
    server::Server,
};

/// If the server does not receive any requests from clients for this duration,
/// the server will automatically exits.
const IDLE_STATE_MAX_SECS: Duration = Duration::from_secs(360);

pub struct EventLoop {
    sock_path: PathBuf,
    progress: Arc<AtomicBool>,
    clients: Arc<tokio::sync::Mutex<ClientSet>>,
    notification_tx: UnboundedSender<Notification>,
    notification_rx: Option<UnboundedReceiver<Notification>>,
}

impl EventLoop {
    pub fn new(sock_path: &Path) -> EventLoop {
        let (notification_tx, notification_rx) = tokio::sync::mpsc::unbounded_channel();
        EventLoop {
            sock_path: sock_path.to_owned(),
            progress: Arc::new(AtomicBool::new(true)),
            clients: Arc::new(tokio::sync::Mutex::new(ClientSet::new())),
            notification_tx,
            notification_rx: Some(notification_rx),
        }
    }

    pub fn notification_tx(&self) -> UnboundedSender<Notification> {
        self.notification_tx.clone()
    }

    pub async fn run(mut self, server: impl Server + 'static) {
        let server = Arc::new(tokio::sync::Mutex::new(server));

        let listener = match UnixListener::bind(&self.sock_path) {
            Ok(listener) => listener,
            Err(err) => {
                error!("Failed to bind to socket: {}", err);
                return;
            }
        };

        // Broadcast notifications from the LSP server to all clients.
        {
            let clients = self.clients.clone();
            let mut notification_rx = self.notification_rx.take().unwrap();
            tokio::spawn(async move {
                while let Some(noti) = notification_rx.recv().await {
                    trace!("sending a notification to noa: {:?}", noti);
                    clients.lock().await.notify(noti).await;
                }
            });
        }

        let (quit_tx, mut quit_rx) = mpsc::channel(1);
        let mut idle_timer = tokio::time::interval(IDLE_STATE_MAX_SECS);
        loop {
            tokio::select! {
                biased;

                _ = quit_rx.recv() => {
                    break;
                }

                Ok((new_client, _)) = listener.accept() => {
                    self.handle_client(new_client, server.clone(), quit_tx.clone())
                        .await;
                }
                _ = idle_timer.tick() => {
                    // Timed out.
                    if !self.progress.load(Ordering::SeqCst) {
                        info!("still in the idle state for a long while, exiting...");
                        return;
                    }

                    // If the server is not idle, progress will be set to true
                    // in next IDLE_STATE_MAX_SECS seconds.
                    self.progress.store(false, Ordering::SeqCst);
                }
            }

            idle_timer.reset();
        }
    }

    /// Spawns a new task to handle a client.
    pub async fn handle_client<S: Server + 'static>(
        &self,
        client: UnixStream,
        server: Arc<tokio::sync::Mutex<S>>,
        quit_tx: mpsc::Sender<()>,
    ) {
        let progress = self.progress.clone();
        let (read_end, write_end) = client.into_split();

        let client_id = self.clients.lock().await.add_client(write_end);
        let clients = self.clients.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(read_end);
            let mut buf = String::with_capacity(128 * 1024);
            loop {
                buf.clear();

                // Receive a request from noa editor.
                match reader.read_line(&mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        progress.store(true, Ordering::SeqCst);

                        let message: ToServer = serde_json::from_str(&buf)
                            .with_context(|| format!("invalid request body: {}", buf))
                            .unwrap();
                        match message {
                            ToServer::Request { id, body } => {
                                let req: <S as Server>::Request =
                                    serde_json::from_value(body).unwrap();
                                let resp = match server.lock().await.process_request(req).await {
                                    Ok(resp) => Response::Ok {
                                        body: serde_json::to_value(resp).unwrap(),
                                    },
                                    Err(err) => {
                                        error!("failed to process request: {}", err);
                                        Response::Err {
                                            reason: err.to_string(),
                                        }
                                    }
                                };
                                clients
                                    .lock()
                                    .await
                                    .send_response(client_id, id, resp)
                                    .await;
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
            info!("client {:?} is being closed", client_id);
            let mut clients = clients.lock().await;
            clients.remove_client(client_id);
            if clients.is_empty() {
                let _ = quit_tx.send(()).await;
            }
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ClientId(usize);

struct ClientSet {
    next_client_id: AtomicUsize,
    clients: HashMap<ClientId, OwnedWriteHalf>,
}

impl ClientSet {
    pub fn new() -> Self {
        ClientSet {
            next_client_id: AtomicUsize::new(1),
            clients: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub fn add_client(&mut self, write_end: OwnedWriteHalf) -> ClientId {
        let client_id = ClientId(self.next_client_id.fetch_add(1, Ordering::SeqCst));
        self.clients.insert(client_id, write_end);
        client_id
    }

    pub fn remove_client(&mut self, id: ClientId) {
        self.clients.remove(&id);
    }

    pub async fn notify(&mut self, noti: Notification) {
        let json = serde_json::to_string(&ToClient::Notification(noti)).unwrap();
        let mut json = json.clone();
        json.push('\n');

        for client in self.clients.values_mut() {
            use tokio::io::AsyncWriteExt;
            client.write_all(json.as_bytes()).await.oops();
        }
    }

    pub async fn send_response(
        &mut self,
        client_id: ClientId,
        request_id: RequestId,
        body: Response,
    ) {
        let json = serde_json::to_string(&ToClient::Response {
            id: request_id,
            body,
        })
        .unwrap();
        let mut json = json.clone();
        json.push('\n');

        let client = self.clients.get_mut(&client_id).unwrap();
        use tokio::io::AsyncWriteExt;
        client.write_all(json.as_bytes()).await.oops();
    }
}
