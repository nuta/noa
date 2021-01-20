use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as TermEvent, MouseButton,
};
pub use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent as RawMouseEvent};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, queue};
use std::cmp::{max, min};
use std::io::{stdout, Write};
use std::thread;
use std::time::Duration;
use unicode_width::UnicodeWidthChar;

pub fn truncate(s: &str, width: usize) -> &str {
    &s[..min(s.chars().count(), width)]
}

fn whitespaces(n: usize) -> String {
    " ".repeat(n)
}

fn num_of_digits(mut n: usize) -> usize {
    match n {
        0..=9 => 1,
        10..=99 => 2,
        100..=999 => 3,
        1000..=9999 => 4,
        10000..=99999 => 5,
        _ => {
            let mut num = 1;
            loop {
                n /= 10;
                if n == 0 {
                    break;
                }
                num += 1;
            }
            num
        }
    }
}

#[derive(Debug)]
pub enum MouseEvent {
    ClickText {
        /// The position in the buffer. They could be larger than the line lengths.
        pos: Point,
        alt: bool,
    },
    ClickLineMap {
        y: usize,
    },
    ScrollUp,
    ScrollDown,
    Drag {
        /// The position in the buffer. They could be larger than the line lengths.
        pos: Point,
    },
}

pub struct Terminal {
    rows: usize,
    cols: usize,
    current_top_left: TopLeft,
    current_num_lines: usize,
    text_start_x: usize,
    text_end_x: usize,
    text_height: usize,
}

impl Terminal {
    pub fn new(event_queue: EventQueue) -> Terminal {
        let (cols, rows) = size().expect("failed to get the terminal size");
        enable_raw_mode().expect("failed to enable the raw mode");
        execute!(stdout(), EnableMouseCapture).expect("failed to enable mouse capture");
        execute!(stdout(), EnterAlternateScreen).expect("failed to enter the alternative screen");

        thread::spawn(move || {
            fn handle_event(event_queue: &EventQueue, ev: TermEvent) {
                match ev {
                    TermEvent::Key(key) => {
                        event_queue.enqueue(Event::Key(key));
                    }
                    TermEvent::Mouse(mouse) => {
                        event_queue.enqueue(Event::Mouse(mouse));
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
            current_top_left: TopLeft::new(0, 0),
            current_num_lines: 0,
            text_start_x: 0,
            text_end_x: 0,
            text_height: 0,
        }
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.rows = rows;
        self.cols = cols;
    }
}
