#![allow(unused)]

use noa_common::logger::install_logger;

mod editor;
mod ui;

#[tokio::main]
async fn main() {
    install_logger("noa");

    let mut editor = editor::Editor::new();
    let mut ui = ui::Ui::new(editor);
    ui.run();
}
