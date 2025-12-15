use std::path::PathBuf;

use crate::{editor::Editor, utils::NOA_DIR};

#[macro_use]
extern crate log;

mod buffer;
mod display_width;
mod editor;
mod logger;
mod status_line;
mod terminal;
mod utils;

fn main() {
    logger::init().expect("failed to initialize logger");

    let path = match std::env::args().nth(1) {
        None => NOA_DIR.join("scratch.txt"),
        Some(path) => PathBuf::from(path),
    };

    let mut editor = Editor::new();
    editor.open_file(path);
    editor.run();
}
