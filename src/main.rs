use std::{
    fs::{File, OpenOptions},
    io::ErrorKind,
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

struct Editor {
    buffer: Buffer,
    status_line: StatusLine,
    terminal: Terminal,
}

impl Editor {
    fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            status_line: StatusLine::new(),
            terminal: Terminal::new(),
        }
    }

    fn open_file(&mut self, path: &PathBuf) {
        self.buffer = match File::open(&path).and_then(Buffer::from_reader) {
            Ok(buffer) => buffer,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // Create a new file.
                Buffer::new()
            }
            Err(e) => {
                error!("failed to open file: {e}");
                return;
            }
        };
    }

    fn run(&mut self) {
        loop {
            use crate::terminal::{Event, KeyCode};

            let frame = self.terminal.frame();
            self.status_line.render(frame, &self.buffer);
            self.terminal.flush();

            let ev = self
                .terminal
                .wait_for_event()
                .expect("failed to wait for event");
            match ev {
                Event::Key(key) => {
                    trace!("key: {}", key.code);
                    if key.code == KeyCode::Char('q') {
                        break;
                    }
                }
                Event::Resize(width, height) => {
                    trace!("resize: {width}x{height}");
                    self.terminal.resize(width, height);
                }
                _ => {
                    warn!("unhandled event: {ev:?}");
                }
            }
        }
    }
}

fn main() {
    logger::init().expect("failed to initialize logger");

    let mut terminal = Terminal::new();
    let status_line = StatusLine::new();

    let path = match std::env::args().nth(1) {
        None => NOA_DIR.join("scratch.txt"),
        Some(path) => PathBuf::from(path),
    };

    let mut editor = Editor::new();
    editor.open_file(&path).expect("failed to open file");
    editor.run();
}
