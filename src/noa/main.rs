use std::path::PathBuf;

use clap::Parser;

use noa_common::logger::install_logger;

mod compositor;
mod document;
mod editor;
mod ui;
mod view;

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    install_logger("main");
    let args = Args::parse();
    let mut editor = editor::Editor::new();
    editor.run().await;
}
