use std::{
    io::{stdout, Stdout, Write},
    time::Duration,
};

use crossterm::{
    cursor::{self, MoveTo},
    event::{
        DisableMouseCapture, EnableMouseCapture, Event as TermEvent, EventStream, KeyCode,
        KeyEvent, KeyModifiers, MouseEvent,
    },
    execute, queue,
    style::{Attribute, Print, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::*,
};
use futures::StreamExt;
use tokio::sync::mpsc::UnboundedSender;

use super::canvas::DrawOp;

#[derive(Debug)]
pub enum Input {
    Key(KeyEvent),
    Mouse(MouseEvent),
    KeyBatch(String),
    Resize {
        screen_height: usize,
        screen_width: usize,
    },
    Redraw,
    Quit,
}

async fn terminal_input_handler(event_queue: UnboundedSender<Input>) {
    let mut stream = EventStream::new().fuse();

    fn handle_event(event_queue: &UnboundedSender<Input>, ev: TermEvent) {
        match ev {
            TermEvent::Key(key) => {
                event_queue.send(Input::Key(key)).ok();
            }
            TermEvent::Mouse(ev) => {
                event_queue.send(Input::Mouse(ev)).ok();
            }
            TermEvent::Resize(cols, rows) => {
                event_queue
                    .send(Input::Resize {
                        screen_width: cols as usize,
                        screen_height: rows as usize,
                    })
                    .ok();
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
                            }
                        }
                    }

                    event_queue.send(Input::KeyBatch(buf)).ok();
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

pub struct Terminal {
    height: usize,
    width: usize,
}

impl Terminal {
    pub fn new(event_queue: UnboundedSender<Input>) -> Terminal {
        enable_raw_mode().expect("failed to enable the raw mode");
        execute!(stdout(), EnterAlternateScreen).expect("failed to enter the alternative screen");
        execute!(stdout(), EnableMouseCapture).expect("failed to enable mouse capture");
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

    pub fn clear(&mut self) {
        execute!(stdout(), Clear(ClearType::All)).ok();
    }

    pub fn drawer(&mut self) -> Drawer {
        let mut stdout = stdout();

        // Hide the cursor to prevent flickering.
        queue!(
            stdout,
            cursor::Hide,
            SetAttribute(Attribute::Reset),
            MoveTo(0, 0),
        )
        .ok();

        Drawer { stdout }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), DisableMouseCapture).ok();
        execute!(stdout(), LeaveAlternateScreen).ok();
        execute!(stdout(), cursor::Show).ok();
        disable_raw_mode().ok();
    }
}

pub struct Drawer {
    stdout: Stdout,
}

impl Drawer {
    pub fn draw(&mut self, op: &DrawOp) {
        match op {
            DrawOp::MoveTo { y, x } => {
                queue!(self.stdout, MoveTo(*x as u16, *y as u16)).ok();
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
            DrawOp::Bold => {
                queue!(self.stdout, SetAttribute(Attribute::Bold)).ok();
            }
            DrawOp::NoBold => {
                queue!(self.stdout, SetAttribute(Attribute::NoBold)).ok();
            }
        }
    }

    pub fn show_cursor(&mut self, screen_y: usize, screen_x: usize) {
        queue!(
            self.stdout,
            MoveTo(screen_x as u16, screen_y as u16),
            cursor::Show
        )
        .ok();
    }

    pub fn flush(&mut self) {
        self.stdout.flush().ok();
    }
}

impl Drop for Drawer {
    fn drop(&mut self) {
        self.flush();
    }
}
