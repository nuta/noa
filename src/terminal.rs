use std::io::{stdout, Write};
use std::thread;
use std::sync::mpsc::{self, Receiver, Sender};
use crossterm::{
    execute, queue,
    event::{self, Event as TermEvent},
    terminal::{
        self,
        enable_raw_mode, disable_raw_mode,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
pub use crossterm::event::{KeyCode, KeyModifiers, KeyEvent};

use crate::editor::Event;

pub struct Terminal {
    rows: usize,
    cols: usize,
}

impl Terminal {
    pub fn new(event_queue: Sender<Event>) -> Terminal {
        let (cols, rows) = terminal::size()
            .expect("failed to get the terminal size");
        enable_raw_mode()
            .expect("failed to enable the raw mode");
        execute!(stdout(), EnterAlternateScreen)
            .expect("failed to enter the alternative screen");

        thread::spawn(move || {
            loop {
                match event::read() {
                    Ok(ev) => {
                        match ev {
                            TermEvent::Key(key) => {
                                event_queue.send(Event::Key(key));
                            }
                            TermEvent::Mouse(mice) => {
                                trace!("unhandled event: {:?}", mice);
                            }
                            TermEvent::Resize(cols, rows) => {
                                event_queue.send(Event::Resize {
                                    cols: cols as usize,
                                    rows: rows as usize,
                                });
                            }
                        }
                    }
                    Err(err) => {
                        warn!("failed to read a terminal event: {}", err);
                    }
                }
            }
        });

        Terminal {
            rows: rows as usize,
            cols: cols as usize,
        }
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.rows = rows;
        self.cols = cols;
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen);
        disable_raw_mode();
    }
}
