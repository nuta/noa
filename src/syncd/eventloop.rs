use anyhow::Result;
use noa_common::warn_on_error;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

pub trait Daemon {
    type Request: DeserializeOwned;
    type Response: Serialize;

    fn process(&mut self, request: Self::Request) -> Result<Self::Response>;
}

pub async fn eventloop<D: Daemon>(mut daemon: D) -> Result<()> {
    let sock_path = "";
    let mut unix_sock = UnixStream::connect(sock_path).await?;
    let (read_end, mut write_end) = unix_sock.split();
    let mut reader = BufReader::new(read_end);
    let mut buf = String::with_capacity(16 * 1024);
    loop {
        buf.clear();

        reader.read_line(&mut buf).await?;
        let request: D::Request = serde_json::from_str(&buf).expect("invalid request");

        let response = daemon.process(request)?;
        warn_on_error!(
            write_end
                .write_all(serde_json::to_string(&response)?.as_bytes())
                .await,
            "failed to write the response"
        );
    }
}
