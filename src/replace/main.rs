#[macro_use]
extern crate log;

use noa_common::logger::install_logger;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
    #[structopt(long, parse(from_os_str))]
    dir: PathBuf,
}

#[tokio::main]
async fn main() {
    install_logger("replace");
    trace!("starting");

    let opt = Opt::from_args();
}
