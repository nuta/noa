#![allow(unused)]

#[macro_use]
extern crate log;

use noa_common::logger::install_logger;
use tokio::{
    fs::{create_dir_all, OpenOptions},
    io::AsyncWriteExt,
    sync::mpsc,
};

mod document;
mod editor;
mod ui;

#[tokio::main]
async fn main() {
    install_logger("main");
    info!("hi!");

    let mut editor = editor::Editor::new();
    let mut ui = ui::Ui::new(editor);
    ui.run().await;
}
