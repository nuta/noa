#[macro_use]
extern crate log;

mod eventloop;
mod lsp;

use noa_common::{logger::install_logger, syncd_protocol::Notification};
use std::path::PathBuf;
use structopt::StructOpt;
use tokio::net::UnixStream;

use crate::{eventloop::eventloop, lsp::LspDaemon};

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
    install_logger();
    trace!("starting");

    let opt = Opt::from_args();

    if UnixStream::connect(&opt.sock_path).await.is_ok() {
        panic!("syncd already running at {}", opt.sock_path.display());
    }

    match opt.daemon_type.as_str() {
        "lsp" => {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Notification>();

            trace!("starting the LSP server");
            let mut daemon = LspDaemon::spawn(tx, &opt.workspace_dir, opt.lang)
                .await
                .expect("failed to start the LSP mode");

            trace!("sending initialize request to LSP");
            daemon
                .initialize()
                .await
                .expect("failed to initialize the LSP server");

            eventloop(&opt.sock_path, daemon, rx).await.unwrap();
        }
        _ => panic!("unknown daemon type: {}", opt.daemon_type),
    };

    trace!("exiting");
}
