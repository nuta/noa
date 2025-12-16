use std::{
    cmp::min,
    fs::File,
    io::{ErrorKind, stdout},
    ops::ControlFlow,
    path::PathBuf,
    time::Instant,
};

use crate::{
    buffer::{Buffer, Position},
    display_width::DisplayWidth,
    status_line::{Level, StatusLine},
    terminal::{self, Style, Terminal},
    utils::measure,
};

pub struct EventLoop {
    cwd: PathBuf,
    path: PathBuf,
    buffer: Buffer,
    top_left: Position,
    status: StatusLine,
    terminal: Terminal,
}

impl EventLoop {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap(),
            path: PathBuf::new(),
            buffer: Buffer::new(),
            top_left: Position::new(0, 0),
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
        use crossterm::cursor;
        use crossterm::queue;

        queue!(stdout(), cursor::Hide).unwrap();

        let frame = &mut self.terminal.frame();

        let status_y = frame.height.saturating_sub(2);
        if status_y < 3 {
            frame.draw_str(status_y, 0, "too small view");
            return;
        }

        let buffer_height = min(
            status_y,
            (self.buffer.num_lines() - self.top_left.line)
                .try_into()
                .unwrap(),
        );

        // Draw the buffer.
        let main_cursor = self.buffer.main_cursor();
        let mut main_cursor_yx = None;
        for y in 0..buffer_height {
            let line = self.top_left.line + y as usize;
            let line_text = self.buffer.line(line);
            let mut x = 0;
            let mut column = 0;
            for chunk in line_text.chunks() {
                for ch in chunk.chars() {
                    let ch_width = ch.display_width();
                    if ch_width == 0 {
                        continue;
                    }

                    if x + ch_width > frame.width {
                        break;
                    }

                    if main_cursor.anchor() == Position::new(line, column) {
                        main_cursor_yx = Some((y, x));
                    }

                    frame.draw_char(y, x, ch, Style::default());
                    x += ch_width;
                    column += 1;
                }
            }

            if main_cursor.anchor() == Position::new(line, column) {
                main_cursor_yx = Some((y, x));
            }
        }

        for y in buffer_height..status_y {
            frame.fill(y, 0, ' ', frame.width, Style::default());
        }

        self.status
            .render(frame, status_y, &self.buffer, &self.cwd, &self.path);

        self.terminal.flush(main_cursor_yx.unwrap());
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
        measure("first-render", || self.render());

        'outer: loop {
            let events = self
                .terminal
                .wait_for_events()
                .expect("failed to wait for event");

            for ev in events {
                if let ControlFlow::Break(_) = measure("event", || self.handle_event(ev)) {
                    break 'outer;
                }
            }

            measure("render", || self.render());
        }
    }
}
