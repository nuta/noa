#![allow(unused)]
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

use clap::Parser;

use daemonize::Daemonize;
use noa_common::{dirs::proxy_pid_path, logger::install_logger};

#[derive(Parser, Debug)]
struct Args {
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
    install_logger("proxy");
    let args = Args::parse();

    if args.daemonize {
        if let Err(err) = Daemonize::new()
            .pid_file(&args.pid_path)
            .working_directory(args.workspace_dir)
            .start()
        {
            panic!("failed to daemonize: {}", err);
        }
    }

    let server = eventloop::EventLoop::new(&args.sock_path);
    server.run().await;
}
