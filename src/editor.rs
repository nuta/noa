use std::{fs::File, io::ErrorKind, path::PathBuf};

use crate::{buffer::Buffer, status_line::StatusLine, terminal::Terminal};

pub struct Editor {
    cwd: PathBuf,
    path: PathBuf,
    buffer: Buffer,
    status_line: StatusLine,
    terminal: Terminal,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap(),
            path: PathBuf::new(),
            buffer: Buffer::new(),
            status_line: StatusLine::new(),
            terminal: Terminal::new(),
        }
    }

    pub fn open_file(&mut self, path: PathBuf) {
        self.path = path;
        self.buffer = match File::open(&self.path).and_then(Buffer::from_reader) {
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

    pub fn run(&mut self) {
        'outer: loop {
            use crate::terminal::{Event, KeyCode};

            let frame = self.terminal.frame();
            self.status_line
                .render(frame, &self.buffer, &self.cwd, &self.path);
            self.terminal.flush();

            let events = self
                .terminal
                .wait_for_events()
                .expect("failed to wait for event");

            for ev in events {
                match ev {
                    Event::Key(key) => {
                        trace!("key: {}", key.code);
                        if key.code == KeyCode::Char('q') {
                            break 'outer;
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
}
