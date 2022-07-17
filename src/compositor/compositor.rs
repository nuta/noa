use std::slice;

use tokio::sync::mpsc;

use crate::{
    canvas::Canvas,
    terminal::{self, Terminal,InputEvent},
};

#[derive(Clone, Copy, Debug)]
pub struct RectSize {
    pub height: usize,
    pub width: usize,
}

pub struct Compositor {
    terminal: Terminal,
    term_rx: mpsc::UnboundedReceiver<terminal::Event>,
    screens: [Canvas; 2],
    screen_size: RectSize,
}

impl Compositor {
    pub fn new() -> Compositor {
        let (term_tx, term_rx) = mpsc::unbounded_channel();
        let terminal = Terminal::new(move |ev| {
            term_tx.send(ev).ok();
        });

        let screen_size = RectSize {
            height: terminal.height(),
            width: terminal.width(),
        };

        Compositor {
            terminal,
            term_rx,
            screens: [
                Canvas::new(screen_size.height, screen_size.width),
                Canvas::new(screen_size.height, screen_size.width),
            ],
            screen_size,
        }
    }

    pub fn screen_size(&self) -> RectSize {
        self.screen_size
    }

    pub async fn recv_terminal_event(&mut self) -> Option<terminal::Event> {
        self.term_rx.recv().await
    }

    pub fn render_to_terminal(&mut self) {}
}
