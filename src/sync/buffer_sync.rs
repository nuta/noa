use std::{sync::mpsc::Receiver, time::Duration};

use anyhow::{bail, Result};
use async_trait::async_trait;
use noa_common::{
    fast_hash::compute_fast_hash,
    sync_protocol::{BufferSyncRequest, BufferSyncResponse, Notification},
};
use notify::{watcher, DebouncedEvent, RecommendedWatcher, Watcher};
use tokio::sync::mpsc::UnboundedSender;

use std::fs::read_to_string;

use crate::eventloop::Daemon;

pub struct BufferSyncDaemon {
    broadcast_tx: UnboundedSender<Notification>,
    fs_watcher: RecommendedWatcher,
}

impl BufferSyncDaemon {
    pub async fn spawn(broadcast_tx: UnboundedSender<Notification>) -> Result<BufferSyncDaemon> {
        let (tx, rx) = std::sync::mpsc::channel();

        tokio::spawn(handle_fs_changes(rx, broadcast_tx.clone()));

        Ok(BufferSyncDaemon {
            broadcast_tx,
            fs_watcher: watcher(tx, Duration::from_millis(20))?,
        })
    }
}

#[async_trait]
impl Daemon for BufferSyncDaemon {
    type Request = BufferSyncRequest;
    type Response = BufferSyncResponse;

    async fn process_request(&mut self, request: Self::Request) -> Result<Self::Response> {
        match request {
            BufferSyncRequest::OpenFile { path } => {
                if let Err(err) = self
                    .fs_watcher
                    .watch(&path, notify::RecursiveMode::NonRecursive)
                {
                    bail!("failed to watch a path '{}': {:?}", path.display(), err);
                }

                Ok(BufferSyncResponse::NoContent)
            }
            BufferSyncRequest::UpdateFile { path, text } => {
                let hash = compute_fast_hash(text.as_bytes());
                self.broadcast_tx
                    .send(Notification::FileModified { path, text, hash })
                    .ok();
                Ok(BufferSyncResponse::NoContent)
            }
            BufferSyncRequest::OpenFileInOther {
                pane_id,
                path,
                position,
            } => {
                self.broadcast_tx
                    .send(Notification::OpenFileInOther {
                        pane_id,
                        path,
                        position,
                    })
                    .ok();
                Ok(BufferSyncResponse::NoContent)
            }
        }
    }
}

unsafe impl Send for BufferSyncDaemon {}

async fn handle_fs_changes(
    rx: Receiver<DebouncedEvent>,
    broadcast_tx: UnboundedSender<Notification>,
) {
    while let Ok(ev) = rx.recv() {
        trace!("fs change detected: {:?}", ev);
        match ev {
            DebouncedEvent::Write(path) => match read_to_string(&path) {
                Ok(text) => {
                    let hash = compute_fast_hash(text.as_bytes());
                    broadcast_tx
                        .send(Notification::FileModified { path, text, hash })
                        .unwrap();
                }
                Err(err) => {
                    warn!("failed to read {}: {:?}", path.display(), err);
                }
            },
            _ => {}
        }
    }

    trace!("handle_fs_changes: exiting");
}
