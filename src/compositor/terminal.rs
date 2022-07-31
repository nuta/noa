use std::{
    fmt,
    io::{stdout, Stdout, Write},
    time::Duration,
};

use crossterm::{
    cursor::{self, MoveTo},
    event::{DisableMouseCapture, Event as TermEvent, EventStream, KeyEvent},
    execute, queue,
    style::{Attribute, Print, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::*,
};
use futures::StreamExt;

pub use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent};

use crate::canvas::DrawOp;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    KeyBatch(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
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

        let mut stdout = stdout();
        queue!(
            stdout,
            EnterAlternateScreen,
            // TODO: EnableMouseCapture,
            // SetCursorShape::BlinkingBeam,
        )
        .ok();
        stdout.flush().ok();

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
        execute!(
            stdout(),
            SetAttribute(Attribute::Reset),
            Clear(ClearType::All)
        )
        .ok();
    }

    pub fn run_in_cooked_mode<R, F: FnOnce() -> R>(&mut self, cb: F) -> R {
        disable_raw_mode().expect("failed to disable the raw mode");
        let output = cb();
        enable_raw_mode().expect("failed to enable the raw mode");
        output
    }

    pub fn drawer(&mut self) -> Drawer<'_> {
        Drawer {
            stdout: stdout(),
            _terminal: self,
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let mut stdout = stdout();
        let _ = execute!(stdout, DisableMouseCapture);
        let _ = execute!(stdout, LeaveAlternateScreen);
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

pub struct Drawer<'a> {
    stdout: Stdout,
    // Keep the terminal reference so that we don't write into stdout after
    // it has been dropped.
    _terminal: &'a Terminal,
}

impl<'a> Drawer<'a> {
    pub fn before_drawing(&mut self) {
        // Hide the cursor to prevent flickering.
        queue!(
            self.stdout,
            SynchronizedOutput::Begin,
            cursor::Hide,
            SetAttribute(Attribute::Reset),
            MoveTo(0, 0),
        )
        .ok();
    }

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
                queue!(self.stdout, SetAttribute(Attribute::NormalIntensity)).ok();
            }
            DrawOp::Invert => {
                queue!(self.stdout, SetAttribute(Attribute::Reverse)).ok();
            }
            DrawOp::NoInvert => {
                queue!(self.stdout, SetAttribute(Attribute::NoReverse)).ok();
            }
            DrawOp::Underline => {
                queue!(self.stdout, SetAttribute(Attribute::Underlined)).ok();
            }
            DrawOp::NoUnderline => {
                queue!(self.stdout, SetAttribute(Attribute::NoUnderline)).ok();
            }
        }
    }

    pub fn flush(&mut self, cursor_pos: Option<(usize, usize)>) {
        if let Some((y, x)) = cursor_pos {
            queue!(self.stdout, MoveTo(x as u16, y as u16), cursor::Show).ok();
        }
        queue!(self.stdout, SynchronizedOutput::End).ok();
        self.stdout.flush().ok();
    }
}

impl<'a> Drop for Drawer<'a> {
    fn drop(&mut self) {
        self.stdout.flush().ok();
    }
}

/// An terminal extension which allows applying multiple changes in the terminal
/// at once not to show intermediate results.
///
/// There're two specifications for this purpose and we support both of them:
///
/// - iTerm2 / Alacritty: <https://gitlab.com/gnachman/iterm2/-/wikis/synchronized-updates-spec>
/// - Contour: <https://gist.github.com/christianparpart/d8a62cc1ab659194337d73e399004036>
pub enum SynchronizedOutput {
    Begin,
    End,
}

impl crossterm::Command for SynchronizedOutput {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        let (param_2026, iterm2_op) = match self {
            SynchronizedOutput::Begin => ('h', '1'),
            SynchronizedOutput::End => ('l', '2'),
        };

        write!(
            f,
            concat!(
                "\x1b[?2026{}",    // CSI ? 2026 param
                "\x1bP={}s\x1b\\"  // ESC P = OP s ESC \
            ),
            param_2026, iterm2_op
        )
    }
}

/// Changes the cursor shape.
///
/// <https://www.vt100.net/docs/vt510-rm/DECSCUSR.html>
pub enum SetCursorShape {
    BlinkingBeam,
}

impl crossterm::Command for SetCursorShape {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        let shape = match self {
            SetCursorShape::BlinkingBeam => '5',
        };

        // CSI shape SP q
        write!(f, "\x1b[{} q", shape)
    }
}
