#![allow(unused)]

use dirs::home_dir;
use log::LevelFilter;
use simplelog::{Config, WriteLogger};
use std::{env::current_dir, fs::OpenOptions, path::PathBuf};
use structopt::StructOpt;

#[macro_use]
extern crate log;
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

mod buffer;
mod editorconfig;
mod eventloop;
mod range_tree;
mod rope;
mod terminal;
mod view;

#[derive(StructOpt)]
struct Opt {
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

pub fn main() {
    WriteLogger::init(
        LevelFilter::Trace,
        Config::default(),
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(home_dir().unwrap().join(".noa.log"))
            .unwrap(),
    )
    .unwrap();

    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        error!("{:#?}", backtrace::Backtrace::new());
    }));

    trace!("starting");

    let opt = Opt::from_args();
    let mut eventloop = eventloop::EventLoop::new(current_dir().unwrap());
    for file in opt.files.iter().rev() {
        eventloop.open_file(file);
    }

    eventloop.run();
}
