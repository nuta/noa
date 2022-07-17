#![allow(unused)]

#[macro_use]
extern crate log;

use noa_common::{logger::install_logger};
use tokio::{sync::mpsc, fs::{OpenOptions, create_dir_all}, io::AsyncWriteExt};

mod editor;
mod ui;
mod document;

#[tokio::main]
async fn main() {
    install_logger("main");
    info!("hi!");

    let mut editor = editor::Editor::new();
    let mut ui = ui::Ui::new(editor);
    ui.run();
}
