use std::{
    fmt,
    io::{stdout, Stdout, Write},
    time::Duration,
};

use crossterm::{
    cursor::{self, MoveTo},
    event::{DisableMouseCapture, EnableMouseCapture, Event as TermEvent, EventStream, KeyEvent},
    execute, queue,
    style::{Attribute, Print, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::*,
};
use futures::{channel::oneshot, StreamExt};

pub use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent};
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};

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
    event_tx: UnboundedSender<Event>,
    stdio_listener: Option<(JoinHandle<()>, oneshot::Sender<()>)>,
}

impl Terminal {
    pub fn new(event_tx: UnboundedSender<Event>) -> Terminal {
        enable_raw_mode().expect("failed to enable the raw mode");

        let mut stdout = stdout();
        queue!(stdout, EnterAlternateScreen, EnableMouseCapture).ok();
        stdout.flush().ok();

        let (event_abort_tx, event_abort_rx) = oneshot::channel();
        let stdio_listener = listen_events(event_tx.clone(), event_abort_rx);

        let (cols, rows) = size().expect("failed to get the terminal size");
        Terminal {
            height: rows as usize,
            width: cols as usize,
            stdio_listener: Some((stdio_listener, event_abort_tx)),
            event_tx,
        }
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub async fn stop_stdin_listening(&mut self) {
        if let Some((join_handle, abort)) = self.stdio_listener.take() {
            abort.send(()).unwrap();
            join_handle.await.unwrap();
            execute!(stdout(), DisableMouseCapture).ok();
            disable_raw_mode().unwrap();
        }
    }

    pub fn restart_stdin_listening(&mut self) {
        debug_assert!(self.stdio_listener.is_none());

        let (event_abort_tx, event_abort_rx) = oneshot::channel();
        let stdio_listener = listen_events(self.event_tx.clone(), event_abort_rx);
        self.stdio_listener = Some((stdio_listener, event_abort_tx));
        enable_raw_mode().unwrap();
        execute!(stdout(), DisableMouseCapture, EnterAlternateScreen).ok();
    }

    pub fn clear(&mut self) {
        execute!(
            stdout(),
            SetAttribute(Attribute::Reset),
            Clear(ClearType::All)
        )
        .ok();
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

fn listen_events(
    event_tx: UnboundedSender<Event>,
    mut abort: oneshot::Receiver<()>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        fn convert_event(ev: TermEvent) -> Event {
            match ev {
                TermEvent::Key(key) => Event::Input(InputEvent::Key(key)),
                TermEvent::Mouse(ev) => Event::Input(InputEvent::Mouse(ev)),
                TermEvent::Resize(cols, rows) => Event::Resize {
                    width: cols as usize,
                    height: rows as usize,
                },
            }
        }

        fn is_next_available() -> bool {
            crossterm::event::poll(Duration::from_secs(0)).unwrap()
        }

        let mut stream = EventStream::new().fuse();
        loop {
            tokio::select! {
                biased;
                Ok(_) = &mut abort => {
                    break;
                }
                    Some(Ok(ev)) = stream.next() => {
                    info!("ev = {:?}", ev);
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

                            let _ = event_tx.send(Event::Input(InputEvent::KeyBatch(buf)));
                            if let Some(ev) = next_event {
                                let _ = event_tx.send(convert_event(ev));
                            }
                        }
                        _ => {
                            let _ = event_tx.send(convert_event(ev));
                        }
                    }
                }
            }
        }
    })
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
        // FIXME:
        return write!(f, "");

        // let (param_2026, iterm2_op) = match self {
        //     SynchronizedOutput::Begin => ('h', '1'),
        //     SynchronizedOutput::End => ('l', '2'),
        // };

        // write!(
        //     f,
        //     concat!(
        //         "\x1b[?2026{}",    // CSI ? 2026 param
        //         "\x1bP={}s\x1b\\"  // ESC P = OP s ESC \
        //     ),
        //     param_2026, iterm2_op
        // )
    }
}
