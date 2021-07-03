#[macro_use]
extern crate log;

mod buffer_sync;
mod eventloop;
mod lsp;

use noa_common::{logger::install_logger, syncd_protocol::Notification};
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
    lang: String,
}

#[tokio::main]
async fn main() {
    install_logger("syncd");
    trace!("starting");

    let opt = Opt::from_args();

    if UnixStream::connect(&opt.sock_path).await.is_ok() {
        panic!("syncd already running at {}", opt.sock_path.display());
    }

    match opt.daemon_type.as_str() {
        "syncd" => {
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
            let mut daemon = LspDaemon::spawn(noti_tx, &opt.workspace_dir, opt.lang)
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
