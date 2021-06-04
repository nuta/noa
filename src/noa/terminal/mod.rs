use std::cmp::min;
use std::io::Write;
use std::ops;
use std::{io::stdout, time::Duration};

pub use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{
    Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::*;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{
    cursor::{self, MoveTo},
    event::EventStream,
};
use crossterm::{event::Event as TermEvent, terminal};
use crossterm::{execute, queue};
use futures::{Stream, StreamExt, TryStreamExt};

use crate::{
    eventloop::{Event, EventQueue},
    surfaces::Context,
    view::View,
};

use noa_buffer::{Buffer, Cursor, Point};

pub mod compositor;
pub mod display_width;

pub struct Terminal {
    screen_height: usize,
    screen_width: usize,
}

async fn terminal_input_handler(event_queue: EventQueue) {
    let mut stream = EventStream::new().fuse();

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
                    screen_width: cols as usize,
                    screen_height: rows as usize,
                });
            }
        }
    }

    fn is_next_available() -> crossterm::Result<bool> {
        crossterm::event::poll(Duration::from_secs(0))
    }

    loop {
        if let Some(Ok(ev)) = stream.next().await {
            match ev {
                TermEvent::Key(KeyEvent {
                    code: KeyCode::Char(key),
                    modifiers: KeyModifiers::NONE,
                }) if is_next_available().unwrap() => {
                    let mut next_event = None;
                    let mut buf = key.to_string();
                    while is_next_available().unwrap() && next_event.is_none() {
                        if let Some(Ok(ev)) = stream.next().await {
                            match ev {
                                TermEvent::Key(KeyEvent {
                                    code: KeyCode::Char(ch),
                                    modifiers: KeyModifiers::SHIFT,
                                }) => {
                                    buf.push(ch);
                                }
                                TermEvent::Key(KeyEvent {
                                    code,
                                    modifiers: KeyModifiers::NONE,
                                }) => match code {
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
                                ev => {
                                    next_event = Some(ev);
                                }
                                _ => {}
                            }
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
}

impl Terminal {
    pub fn new(event_queue: EventQueue) -> Terminal {
        enable_raw_mode().expect("failed to enable the raw mode");
        execute!(stdout(), EnterAlternateScreen).expect("failed to enter the alternative screen");
        tokio::spawn(terminal_input_handler(event_queue));
        let (cols, rows) = size().expect("failed to get the terminal size");
        Terminal {
            screen_height: rows as usize,
            screen_width: cols as usize,
        }
    }

    pub fn draw(&mut self, ctx: &Context) {
        let mut stdout = stdout();
        if self.screen_width < 10 || self.screen_height < 5 {
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
