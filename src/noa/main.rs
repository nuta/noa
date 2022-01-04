use std::path::PathBuf;

use clap::Parser;

use noa_common::logger::install_logger;

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    install_logger("main");
    let args = Args::parse();
}
