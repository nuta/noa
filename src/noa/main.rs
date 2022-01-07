#![allow(unused)]
#![feature(test)]

extern crate test;

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

use std::path::PathBuf;

use clap::Parser;

use noa_common::logger::install_logger;
use ui::{compositor::Compositor, terminal::Terminal};

mod document;
mod editor;
mod highlighting;
mod ui;
mod view;

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    install_logger("main");
    let args = Args::parse();

    let terminal = Terminal::new();
    let compositor = Compositor::new(terminal);
    let mut editor = editor::Editor::new(compositor);
    editor.run().await;
}
