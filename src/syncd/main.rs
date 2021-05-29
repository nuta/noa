#[macro_use]
extern crate log;

mod eventloop;
mod lsp;

use dirs::home_dir;
use log::LevelFilter;
use simplelog::{CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
use std::fs::OpenOptions;
use structopt::StructOpt;

use crate::{eventloop::eventloop, lsp::LspDaemon};

#[derive(StructOpt)]
struct Opt {
    #[structopt(long)]
    workspace_dir: String,
    #[structopt(long, name = "type")]
    daemon_type: String,
    #[structopt(long)]
    lang: String,
}

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
                .open(home_dir().unwrap().join(".noa-syncd.log"))
                .unwrap(),
        ),
    ])
    .unwrap();

    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    trace!("starting");

    let opt = Opt::from_args();
    let daemon = match opt.daemon_type.as_str() {
        "lsp" => eventloop(LspDaemon::new()),
        _ => panic!("unknown daemon type: {}", opt.daemon_type),
    }
    .await
    .unwrap();

    trace!("exiting");
}
