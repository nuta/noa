use std::cmp::min;
use std::io::Stdout;
use std::io::Write;
use std::ops;
use std::{io::stdout, time::Duration};

pub use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::Attributes;
use crossterm::style::SetAttributes;
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
use tokio::sync::mpsc::UnboundedSender;

use crate::terminal::compositor::Event;
use crate::{surfaces::Context, view::View};

use noa_buffer::{Buffer, Cursor, Point};

pub mod canvas;
pub mod compositor;
pub mod display_width;

async fn terminal_input_handler(event_queue: UnboundedSender<Event>) {
    let mut stream = EventStream::new().fuse();

    fn handle_event(event_queue: &UnboundedSender<Event>, ev: TermEvent) {
        match ev {
            TermEvent::Key(key) => {
                event_queue.send(Event::Key(key));
            }
            TermEvent::Mouse(_) => {
                unreachable!();
            }
            TermEvent::Resize(cols, rows) => {
                event_queue.send(Event::Resize {
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

                    event_queue.send(Event::KeyBatch(buf));
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

pub enum DrawOp<'a> {
    MoveTo { y: usize, x: usize },
    Grapheme(&'a str),
    FgColor(Color),
    BgColor(Color),
    Attributes(Attributes),
    Reset,
}

pub struct Terminal {
    height: usize,
    width: usize,
}

impl Terminal {
    pub fn new(event_queue: UnboundedSender<Event>) -> Terminal {
        enable_raw_mode().expect("failed to enable the raw mode");
        execute!(stdout(), EnterAlternateScreen).expect("failed to enter the alternative screen");
        tokio::spawn(terminal_input_handler(event_queue));
        let (cols, rows) = size().expect("failed to get the terminal size");
        Terminal {
            height: rows as usize,
            width: cols as usize,
        }
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn drawer(&mut self) -> Option<Drawer> {
        let mut stdout = stdout();
        if self.width < 10 || self.height < 5 {
            queue!(
                stdout,
                Clear(ClearType::All),
                MoveTo(0, 0),
                Print("too small!"),
            )
            .unwrap();
            stdout.flush().unwrap();
            return None;
        }

        // Hide the cursor to prevent flickering.
        queue!(stdout, cursor::Hide, Clear(ClearType::All), MoveTo(0, 0),).ok();
        Some(Drawer { stdout })
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen).ok();
        execute!(stdout(), cursor::Show).ok();
        disable_raw_mode().ok();
    }
}

pub struct Drawer {
    stdout: Stdout,
}

impl Drawer {
    fn draw(&mut self, op: &DrawOp) {
        match op {
            DrawOp::MoveTo { y, x } => {
                queue!(self.stdout, MoveTo(*y as u16, *x as u16)).ok();
            }
            DrawOp::Grapheme(s) => {
                queue!(self.stdout, Print(s)).ok();
            }
            DrawOp::FgColor(color) => {
                queue!(self.stdout, SetForegroundColor(*color)).ok();
            }
            DrawOp::BgColor(color) => {
                queue!(self.stdout, SetBackgroundColor(*color)).ok();
            }
            DrawOp::Reset => {
                queue!(self.stdout, Clear(ClearType::All)).ok();
            }
            DrawOp::Attributes(attrs) => {
                queue!(self.stdout, SetAttributes(*attrs)).ok();
            }
        }
    }
}

impl Drop for Drawer {
    fn drop(&mut self) {
        self.stdout.flush().ok();
    }
}
