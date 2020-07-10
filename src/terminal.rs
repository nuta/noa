use crate::editor::Event;
use crate::buffer::Buffer;
use std::io::{stdout, Write};
use std::thread;
use std::sync::mpsc::{self, Receiver, Sender};
pub use crossterm::event::{KeyCode, KeyModifiers, KeyEvent};
use crossterm::event::{self, Event as TermEvent};
use crossterm::{execute, queue};
use crossterm::terminal::{
    size, enable_raw_mode, disable_raw_mode,
    EnterAlternateScreen, LeaveAlternateScreen,
};

pub struct Terminal {
    rows: u16,
    cols: u16,
}

impl Terminal {
    pub fn new(event_queue: Sender<Event>) -> Terminal {

        let (cols, rows) = size()
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
                                event_queue.send(Event::Resize { cols, rows });
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
            rows,
            cols,
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
    }

    pub fn draw(&mut self, buffer: &Buffer) {
        use crossterm::cursor::{MoveTo};
        use crossterm::style::{Print};
        use crossterm::terminal::{Clear, ClearType};

        let mut stdout = stdout();
        if self.cols < 5 || self.rows < 5 {
            queue!(stdout,
                Clear(ClearType::All),
                MoveTo(0, 0),
                Print("too small!"),
            );
            stdout.flush();
            return;
        }


        stdout.flush();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen);
        disable_raw_mode();
    }
}
