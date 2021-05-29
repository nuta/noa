#[macro_use]
extern crate log;

mod eventloop;

use dirs::home_dir;
use log::LevelFilter;
use simplelog::{CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
use std::fs::OpenOptions;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {}

#[tokio::main]
async fn main() {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Trace,
            Config::default(),
            TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Trace,
            Config::default(),
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(home_dir().unwrap().join(".noa-lspd.log"))
                .unwrap(),
        ),
    ])
    .unwrap();

    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    trace!("starting");

    let _opt = Opt::from_args();
    let mut ev = eventloop::EventLoop::new();
    ev.run().await;
}
