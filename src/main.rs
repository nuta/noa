use dirs::home_dir;
use log::LevelFilter;
use simplelog::{Config, WriteLogger};
use std::fs::OpenOptions;

#[macro_use]
extern crate log;

mod buffer;
mod editorconfig;
mod rope;

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
}
