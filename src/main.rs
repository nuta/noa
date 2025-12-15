use std::{
    fs::OpenOptions,
    path::PathBuf,
};

use crate::{buffer::Buffer, status_line::StatusLine, terminal::Terminal, utils::NOA_DIR};

#[macro_use]
extern crate log;

mod buffer;
mod logger;
mod status_line;
mod terminal;
mod utils;

fn main() {
    logger::init().expect("failed to initialize logger");

    let mut terminal = Terminal::new();
    let status_line = StatusLine::new();

    let path = match std::env::args().nth(1) {
        None => NOA_DIR.join("noa/noa.txt"),
        Some(path) => PathBuf::from(path),
    };

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
        .expect("failed to open file");

    let buffer = Buffer::from_reader(file).expect("failed to read buffer");

    loop {
        use terminal::Event;
        use terminal::KeyCode;

        terminal.render(&[&status_line]);

        let ev = terminal.wait_for_event().expect("failed to wait for event");
        match ev {
            Event::Key(key) => {
                trace!("key: {}", key.code);
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
            Event::Resize(width, height) => {
                trace!("resize: {width}x{height}");
                terminal.resize(width, height);
            }
            _ => {
                warn!("unhandled event: {ev:?}");
            }
        }
    }
}
