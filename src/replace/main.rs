#[macro_use]
extern crate log;

mod path_scanner;
mod search_query;
mod ui;

use noa_common::logger::install_logger;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::{path_scanner::PathScanner, ui::Ui};

#[derive(StructOpt)]
struct Opt {
    #[structopt(long, parse(from_os_str))]
    dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    install_logger("replace");
    trace!("starting");

    let opt = Opt::from_args();
    let ui = Ui::new();
    let base_dir = opt.dir.unwrap_or_else(|| std::env::current_dir().unwrap());

    let path_scanner = PathScanner::new(&base_dir);
    path_scanner.scan(Box::new(|path: PathBuf| {
        println!("{}", path.display());
        true
    }));
}
