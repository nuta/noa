use crate::{status_line::StatusLine, terminal::Terminal};

#[macro_use]
extern crate log;

mod buffer;
mod logger;
mod terminal;
mod status_line;

fn main() {
    logger::init().expect("failed to initialize logger");

    let mut terminal = Terminal::new();
    let mut status_line = StatusLine::new();

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
