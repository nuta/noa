#[macro_use]
extern crate log;

mod eventloop;
mod lsp;

use dirs::home_dir;
use log::LevelFilter;
use noa_common::dirs::lsp_sock_path;
use simplelog::{CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
use std::{fs::OpenOptions, path::PathBuf};
use structopt::StructOpt;

use crate::{eventloop::eventloop, lsp::LspDaemon};

#[derive(StructOpt)]
struct Opt {
    #[structopt(long, parse(from_os_str))]
    workspace_dir: PathBuf,
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
    let (sock_path, daemon) = match opt.daemon_type.as_str() {
        "lsp" => {
            let sock_path = lsp_sock_path(&opt.workspace_dir, &opt.lang);
            let daemon =
                LspDaemon::new(&opt.workspace_dir, opt.lang).expect("failed to start the LSP mode");
            (sock_path, daemon)
        }
        _ => panic!("unknown daemon type: {}", opt.daemon_type),
    };

    eventloop(&sock_path, daemon).await.unwrap();
    trace!("exiting");
}
