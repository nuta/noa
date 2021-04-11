use std::io::Write;
use std::{io::stdout, time::Duration};

use crossterm::cursor::{self, MoveTo};
use crossterm::event::{self, Event as TermEvent};
pub use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::*;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{execute, queue};

use crate::eventloop::{Event, EventQueue};

pub struct DrawContext {}

pub struct Terminal {
    rows: usize,
    cols: usize,
}

impl Terminal {
    pub fn new(event_queue: EventQueue) -> Terminal {
        let (cols, rows) = size().expect("failed to get the terminal size");
        enable_raw_mode().expect("failed to enable the raw mode");
        execute!(stdout(), EnterAlternateScreen).expect("failed to enter the alternative screen");

        std::thread::spawn(move || {
            fn handle_event(event_queue: &EventQueue, ev: TermEvent) {
                match ev {
                    TermEvent::Key(key) => {
                        event_queue.enqueue(Event::Key(key));
                    }
                    TermEvent::Mouse(_) => {
                        unreachable!();
                    }
                    TermEvent::Resize(cols, rows) => {
                        event_queue.enqueue(Event::Resize {
                            cols: cols as usize,
                            rows: rows as usize,
                        });
                    }
                }
            }

            fn is_next_available() -> crossterm::Result<bool> {
                event::poll(Duration::from_secs(0))
            }

            loop {
                if let Ok(ev) = event::read() {
                    match ev {
                        TermEvent::Key(KeyEvent {
                            code: KeyCode::Char(key),
                            modifiers: KeyModifiers::NONE,
                        }) if is_next_available().unwrap() => {
                            let mut next_event = None;
                            let mut buf = key.to_string();
                            while is_next_available().unwrap() && next_event.is_none() {
                                match event::read() {
                                    Ok(TermEvent::Key(KeyEvent {
                                        code: KeyCode::Char(ch),
                                        modifiers: KeyModifiers::SHIFT,
                                    })) => {
                                        buf.push(ch);
                                    }
                                    Ok(TermEvent::Key(KeyEvent {
                                        code,
                                        modifiers: KeyModifiers::NONE,
                                    })) => match code {
                                        KeyCode::Char(ch) => {
                                            buf.push(ch);
                                        }
                                        KeyCode::Enter => {
                                            buf.push('\n');
                                        }
                                        KeyCode::Tab => {
                                            buf.push('\t');
                                        }
                                        _ => {
                                            next_event = Some(ev);
                                        }
                                    },
                                    Ok(ev) => {
                                        next_event = Some(ev);
                                    }
                                    _ => {}
                                }
                            }

                            event_queue.enqueue(Event::KeyBatch(buf));
                            if let Some(ev) = next_event {
                                handle_event(&event_queue, ev);
                            }
                        }
                        _ => {
                            handle_event(&event_queue, ev);
                        }
                    }
                }
            }
        });

        Terminal {
            rows: rows as usize,
            cols: cols as usize,
        }
    }

    pub fn draw(&mut self, ctx: DrawContext) {
        let mut stdout = stdout();
        if self.cols < 10 || self.rows < 5 {
            queue!(
                stdout,
                Clear(ClearType::All),
                MoveTo(0, 0),
                Print("too small!"),
            )
            .unwrap();
            stdout.flush().unwrap();
            return;
        }

        // Hide the cursor to prevent flickering.
        queue!(stdout, cursor::Hide).ok();

        stdout.flush().ok();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen).ok();
        execute!(stdout(), cursor::Show).ok();
        disable_raw_mode().ok();
    }
}
