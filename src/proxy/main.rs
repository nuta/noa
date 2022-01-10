#![feature(test)]

extern crate test;

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

mod protocol;

use clap::Parser;

use noa_common::logger::install_logger;

#[derive(Parser, Debug)]
struct Args {}

#[tokio::main]
async fn main() {
    install_logger("proxy");
    let _args = Args::parse();
}
