#![allow(unused)]
#![feature(test)]
#![feature(vec_retain_mut)]

extern crate test;

#[macro_use]
extern crate log;

use std::path::PathBuf;

use application::Application;
use clap::Parser;

use finder::open_finder;
use noa_common::{logger::install_logger, time_report::TimeReport};

use theme::parse_default_theme;

#[macro_use]
mod notification;

mod actions;
mod application;
mod clipboard;
mod completion;
mod document;
mod editor;
mod event_listener;
mod file_watch;
mod finder;
mod flash;
mod git;
mod hook;
mod job;
mod keybindings;
mod linemap;
mod movement;
mod plugin;
mod plugins;
mod theme;
mod ui;
mod view;

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    let _boot_time = TimeReport::new("boot time");

    // Parse the default theme here to print panics in stderr.
    parse_default_theme();

    install_logger("main");
    let args = Args::parse();

    let workspace_dir = args
        .files
        .iter()
        .find(|path| path.is_dir())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));

    let mut app = Application::new(&workspace_dir, &args.files);
    app.run().await;
}
