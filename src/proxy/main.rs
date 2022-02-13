#![feature(test)]

extern crate test;

#[macro_use]
extern crate log;

mod client;
mod eventloop;
mod lsp;
mod protocol;
mod server;

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use daemonize::Daemonize;
use lsp::LspServer;
use noa_common::{dirs::proxy_pid_path, logger::install_logger};
use noa_languages::definitions::get_language_by_lsp_id;

#[derive(Parser, Debug)]
struct Args {
    #[clap(long)]
    mode: String,
    #[clap(long)]
    lsp_language_id: Option<String>,
    #[clap(long)]
    daemonize: bool,
    #[clap(long, parse(from_os_str))]
    workspace_dir: PathBuf,
    #[clap(long, parse(from_os_str))]
    sock_path: PathBuf,
    #[clap(long, parse(from_os_str))]
    pid_path: PathBuf,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    trace!("starting proxy...");
    let args = Args::parse();

    if args.daemonize {
        if let Err(err) = Daemonize::new()
            .pid_file(&args.pid_path)
            .working_directory(&args.workspace_dir)
            .start()
        {
            panic!("failed to daemonize: {}", err);
        }
    }

    let eventloop = eventloop::EventLoop::new(&args.sock_path);
    let server = match args.mode.as_str() {
        "lsp" => {
            let lang_id = args
                .lsp_language_id
                .as_ref()
                .expect("--lsp-language-id is not set");
            let lsp = get_language_by_lsp_id(lang_id)
                .with_context(|| format!("unsupported lsp language id {}", lang_id))
                .unwrap();

            LspServer::spawn(eventloop.notification_tx(), lsp, &args.workspace_dir)
                .await
                .with_context(|| format!("failed to spawn LSP server for {}", lang_id))
                .unwrap()
        }
        _ => {
            panic!("unsupported mode: {}", args.mode);
        }
    };

    eventloop.run(server).await;
}
