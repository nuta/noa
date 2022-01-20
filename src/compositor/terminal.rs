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

use super::canvas::DrawOp;

#[derive(Clone, PartialEq, Debug)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    KeyBatch(String),
}

#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    Input(InputEvent),
    Resize { height: usize, width: usize },
}

pub struct Terminal {
    height: usize,
    width: usize,
}

impl Terminal {
    pub fn new<F>(event_handler: F) -> Terminal
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        enable_raw_mode().expect("failed to enable the raw mode");
        execute!(stdout(), EnterAlternateScreen).expect("failed to enter the alternative screen");
        execute!(stdout(), EnableMouseCapture).expect("failed to enable mouse capture");
        let (cols, rows) = size().expect("failed to get the terminal size");
        listen_events(event_handler);
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

fn listen_events<F>(event_handler: F)
where
    F: Fn(Event) + Send + Sync + 'static,
{
    tokio::spawn(async move {
        fn handle_event<F>(event_handler: F, ev: TermEvent)
        where
            F: Fn(Event) + Send + Sync,
        {
            match ev {
                TermEvent::Key(key) => {
                    event_handler(Event::Input(InputEvent::Key(key)));
                }
                TermEvent::Mouse(ev) => {
                    event_handler(Event::Input(InputEvent::Mouse(ev)));
                }
                TermEvent::Resize(cols, rows) => {
                    event_handler(Event::Resize {
                        width: cols as usize,
                        height: rows as usize,
                    });
                }
            }
        }

        fn is_next_available() -> bool {
            crossterm::event::poll(Duration::from_secs(0)).unwrap()
        }

        let mut stream = EventStream::new().fuse();
        loop {
            if let Some(Ok(ev)) = stream.next().await {
                match ev {
                    TermEvent::Key(KeyEvent {
                        code: KeyCode::Char(key),
                        modifiers: KeyModifiers::NONE,
                    }) if is_next_available() => {
                        let mut next_event = None;
                        let mut buf = key.to_string();
                        while is_next_available() && next_event.is_none() {
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

                        event_handler(Event::Input(InputEvent::KeyBatch(buf)));
                        if let Some(ev) = next_event {
                            handle_event(&event_handler, ev);
                        }
                    }
                    _ => {
                        handle_event(&event_handler, ev);
                    }
                }
            }
        }
    });
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
