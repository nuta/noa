#![allow(unused)]
#![feature(test)]

extern crate test;

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

use std::path::PathBuf;

use clap::Parser;

use noa_common::logger::install_logger;

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    install_logger("proxy");
    let args = Args::parse();
}
