#[macro_use]
extern crate log;

mod buffer;
mod logger;
mod terminal;

fn main() {
    logger::init().expect("failed to initialize logger");

    let mut terminal = terminal::Terminal::new(80, 24);
    terminal.render();

    loop {
        use terminal::Event;
        use terminal::KeyCode;

        let ev = terminal.wait_for_event().expect("failed to wait for event");
        match ev {
            Event::Key(key) => {
                trace!("key: {:?}", key);
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
            _ => {
                warn!("unhandled event: {:?}", ev);
            }
        }

        terminal.render();
    }
}
