#![allow(unused)]

#[macro_use]
extern crate log;

use noa_common::logger::install_logger;
use tokio::{
    fs::{create_dir_all, OpenOptions},
    io::AsyncWriteExt,
    sync::mpsc,
};

use crate::finder::warm_up_search_cache;

mod document;
mod editor;
mod finder;
mod ui;

#[tokio::main]
async fn main() {
    install_logger("main");

    // warm_up_search_cache();

    let mut editor = editor::Editor::new();
    let mut ui = ui::Ui::new(editor);
    ui.run().await;
}
