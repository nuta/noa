use std::{fs::File, io::ErrorKind, ops::ControlFlow, path::PathBuf, time::Instant};

use crate::{
    buffer::Buffer,
    display_width::DisplayWidth,
    status_line::{Level, StatusLine},
    terminal::{self, Terminal},
};

pub struct Editor {
    cwd: PathBuf,
    path: PathBuf,
    buffer: Buffer,
    status: StatusLine,
    terminal: Terminal,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap(),
            path: PathBuf::new(),
            buffer: Buffer::new(),
            status: StatusLine::new(),
            terminal: Terminal::new(),
        }
    }

    pub fn open_file(&mut self, path: PathBuf) {
        self.path = path;
        self.buffer = match File::open(&self.path).and_then(Buffer::from_reader) {
            Ok(buffer) => {
                self.status
                    .message(Level::Info, format!("{} lines", buffer.num_lines()));
                buffer
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // Create a new file.
                self.status.message(Level::Info, "new file");
                Buffer::new()
            }
            Err(e) => {
                error!("failed to open file: {e}");
                return;
            }
        };
    }

    pub fn render(&mut self) {
        let frame = &mut self.terminal.frame();

        let y = frame.height.saturating_sub(2);
        if y < 3 {
            frame.draw_str(y, 0, "too small view");
            return;
        }

        for y in 0..frame.height {
            let line = y as usize;
            if line >= self.buffer.num_lines() {
                break;
            }

            let line = self.buffer.line(line);
            let mut x = 0;
            for chunk in line.chunks() {
                frame.draw_str(y, x, chunk);
                x += chunk.display_width(); // TODO: cache
            }
        }

        self.status
            .render(frame, y, &self.buffer, &self.cwd, &self.path);
        self.terminal.flush();
    }

    pub fn handle_event(&mut self, ev: terminal::Event) -> ControlFlow<()> {
        use crate::terminal::{Event, KeyCode};
        use crossterm::event::KeyModifiers;

        match ev {
            Event::Key(key) => {
                trace!("key: {} ({:?})", key.code, key.modifiers);
                match (key.modifiers, key.code) {
                    (KeyModifiers::CONTROL, KeyCode::Char('q')) => {
                        return ControlFlow::Break(());
                    }
                    (KeyModifiers::SHIFT | KeyModifiers::NONE, KeyCode::Char(ch)) => {
                        self.buffer.insert_char(ch);
                    }
                    _ => {
                        warn!("unhandled key: {key:?}");
                    }
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

        ControlFlow::Continue(())
    }

    pub fn run(&mut self) {
        loop {
            let render_dur = {
                let started_at = Instant::now();
                self.render();
                started_at.elapsed()
            };

            let events = self
                .terminal
                .wait_for_events()
                .expect("failed to wait for event");

            let num_events = events.len();
            let events_dur = {
                let started_at = Instant::now();
                for ev in events {
                    if let ControlFlow::Break(_) = self.handle_event(ev) {
                        break;
                    }
                }
                started_at.elapsed()
            };

            trace!(
                "iteration: took {:?} (render: {:?}, handle: {:?}, events: {})",
                render_dur + events_dur,
                render_dur,
                events_dur,
                num_events,
            );
        }
    }
}
