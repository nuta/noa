use std::{io::ErrorKind, path::Path};

use anyhow::Result;
use async_trait::async_trait;
use noa_common::warn_on_error;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    sync::mpsc::UnboundedReceiver,
};

#[async_trait]
pub trait Daemon {
    type Request: DeserializeOwned;
    type Notification: Serialize;
    async fn process_request(&mut self, request: Self::Request) -> Result<()>;
}

pub async fn eventloop<D: Daemon>(
    sock_path: &Path,
    mut daemon: D,
    mut noti_rx: UnboundedReceiver<D::Notification>,
) -> Result<()> {
    let mut unix_sock = match UnixStream::connect(sock_path).await {
        Ok(sock) => sock,
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            warn!("{} already exists", sock_path.display());
            return Err(err.into());
        }
        Err(err) => {
            return Err(err.into());
        }
    };

    let (read_end, mut write_end) = unix_sock.split();
    let mut reader = BufReader::new(read_end);
    let mut buf = String::with_capacity(16 * 1024);
    'outer: loop {
        buf.clear();

        tokio::select! {
            Ok(len) = reader.read_line(&mut buf) => {
                if len == 0 {
                    break 'outer;
                }

                let request: D::Request = serde_json::from_str(&buf).expect("invalid request");
                daemon.process_request(request).await?;
            }
            noti = noti_rx.recv() => {
                //
            }
        }

        // warn_on_error!(
        //     write_end
        //         .write_all(serde_json::to_string(&response)?.as_bytes())
        //         .await,
        //     "failed to write the response"
        // );
    }

    Ok(())
}
