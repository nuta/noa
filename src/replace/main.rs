#[macro_use]
extern crate log;

mod ui;

use noa_common::logger::install_logger;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::ui::Ui;

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

    use ignore::{WalkBuilder, WalkState};
    let base_dir = opt.dir.unwrap_or_else(|| std::env::current_dir().unwrap());
    WalkBuilder::new(base_dir).build_parallel().run(|| {
        Box::new(|dirent| {
            if let Ok(dirent) = dirent {
                let meta = dirent.metadata().unwrap();
                if !meta.is_file() {
                    return WalkState::Continue;
                }

                println!("{}", dirent.path().display());
            }
            WalkState::Continue
        })
    });
}
