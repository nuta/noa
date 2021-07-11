#[macro_use]
extern crate log;

mod buffer_sync;
mod eventloop;
mod lsp;

use noa_common::{logger::install_logger, sync_protocol::Notification};
use std::path::PathBuf;
use structopt::StructOpt;
use tokio::net::UnixStream;

use crate::{buffer_sync::BufferSyncDaemon, eventloop::eventloop, lsp::LspDaemon};

#[derive(StructOpt)]
struct Opt {
    #[structopt(long, parse(from_os_str))]
    workspace_dir: PathBuf,
    #[structopt(long, parse(from_os_str))]
    sock_path: PathBuf,
    #[structopt(long, name = "type")]
    daemon_type: String,
    #[structopt(long)]
    lsp_lang: Option<String>,
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();

    install_logger(&format!(
        "sync-{}",
        opt.lsp_lang
            .as_ref()
            .map(String::as_str)
            .unwrap_or_else(|| "buffer")
    ));
    trace!("starting");

    if UnixStream::connect(&opt.sock_path).await.is_ok() {
        panic!("sync already running at {}", opt.sock_path.display());
    }

    match opt.daemon_type.as_str() {
        "buffer_sync" => {
            trace!("starting the buffer_sync server");
            let (noti_tx, noti_rx) = tokio::sync::mpsc::unbounded_channel::<Notification>();

            trace!("starting the LSP server");
            let daemon = BufferSyncDaemon::spawn(noti_tx)
                .await
                .expect("failed to start the LSP mode");

            eventloop(&opt.sock_path, daemon, noti_rx).await.unwrap();
        }
        "lsp" => {
            let (noti_tx, noti_rx) = tokio::sync::mpsc::unbounded_channel::<Notification>();

            trace!("starting the LSP server");
            let mut daemon = LspDaemon::spawn(
                noti_tx,
                &opt.workspace_dir,
                opt.lsp_lang.expect("--lsp-lang is required"),
            )
            .await
            .expect("failed to start the LSP mode");

            trace!("sending initialize request to LSP");
            daemon
                .initialize()
                .await
                .expect("failed to initialize the LSP server");

            eventloop(&opt.sock_path, daemon, noti_rx).await.unwrap();
        }
        _ => panic!("unknown daemon type: {}", opt.daemon_type),
    };

    trace!("exiting");
}
