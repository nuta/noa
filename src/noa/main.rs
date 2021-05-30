#![allow(unused)]

use log::LevelFilter;
use noa_common::dirs::log_file_path;
use simplelog::{Config, WriteLogger};
use std::{env::current_dir, fs::OpenOptions, path::PathBuf};
use structopt::StructOpt;

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

mod eventloop;
mod syncd;
mod terminal;
mod view;

#[derive(StructOpt)]
struct Opt {
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    WriteLogger::init(
        LevelFilter::Trace,
        Config::default(),
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_file_path("noa"))
            .unwrap(),
    )
    .unwrap();

    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    trace!("starting");

    let opt = Opt::from_args();
    let workspace_dir = match opt.files.get(0) {
        Some(file_or_dir) if file_or_dir.is_dir() => file_or_dir.clone(),
        _ => current_dir().unwrap(),
    };

    let mut eventloop = eventloop::EventLoop::new(workspace_dir);
    for file in opt.files.iter().rev() {
        eventloop.open_file(file);
    }

    eventloop.run().await;
}
