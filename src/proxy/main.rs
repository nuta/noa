#![allow(unused)]
#![feature(test)]

extern crate test;

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

mod client;
mod protocol;
mod server;

use std::path::PathBuf;

use clap::Parser;

use noa_common::logger::install_logger;

#[derive(Parser, Debug)]
struct Args {
    #[clap(long, parse(from_os_str))]
    sock_path: PathBuf,
}

#[tokio::main]
async fn main() {
    install_logger("proxy");
    let args = Args::parse();

    let server = server::Server::new(&args.sock_path);
    server.run().await;
}
