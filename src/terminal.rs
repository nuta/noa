use crate::editor::Event;
use crate::rope::Cursor;
use crate::view::View;
use std::cell::RefCell;
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

static mut WHITESPACES: String = String::new();

fn whitespaces(n: usize) -> &'static str {
    // It's safe since this function will be called only in the single-threaded
    // main loop.
    unsafe {
        if WHITESPACES.len() < n {
            WHITESPACES = " ".repeat(n);
        }

        &WHITESPACES[0..n]
    }
}

fn num_of_digits(mut n: usize) -> usize {
    match n {
        0..=9 => 1,
        10..=99 => 2,
        100..=999 => 3,
        1000..=9999 => 4,
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

pub struct Terminal {
    rows: usize,
    cols: usize,
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

    pub fn draw(&mut self, view: &View) {
        use unicode_width::{UnicodeWidthStr, UnicodeWidthChar};
        use crossterm::cursor::{self, MoveTo, MoveDown};
        use crossterm::terminal::{Clear, ClearType};
        use crossterm::style::{
            Print, Color, SetForegroundColor, SetBackgroundColor,
            Attribute, SetAttribute
        };

        let mut stdout = stdout();
        if self.cols < 10 || self.rows < 5 {
            queue!(stdout,
                Clear(ClearType::All),
                MoveTo(0, 0),
                Print("too small!"),
            );
            stdout.flush();
            return;
        }

        let buffer = view.buffer().borrow();
        let top_left = view.top_left();
        let lineno_width = num_of_digits(buffer.num_lines()) + 2;
        let text_height = self.rows - 2;
        let text_width = self.cols - (2 + lineno_width);

        // Draw buffer contents.
        for i in 0..text_height {
            queue!(stdout, MoveTo(0, i as u16));

            // Line number.
            let lineno = top_left.y + i + 1;
            let out_of_bounds = lineno > buffer.num_lines();
            if out_of_bounds {
                queue!(stdout,
                    SetBackgroundColor(Color::AnsiValue(240)),
                    Print(whitespaces(lineno_width)),
                    SetAttribute(Attribute::Reset),
                );
            } else {
                queue!(stdout,
                    SetBackgroundColor(Color::AnsiValue(236)),
                    Print(whitespaces(lineno_width - num_of_digits(lineno) - 1)),
                    Print(lineno),
                    Print(" "),
                    SetAttribute(Attribute::Reset),
                );
            }

            // Line map.
            // TODO:
            queue!(stdout, Print(' '));


            // Text.
            if !out_of_bounds {
                let mut remaining = text_width;
                let slice = buffer.line(top_left.y + i);
                'outer: for chunk in slice.chunks() {
                    let width = UnicodeWidthStr::width_cjk(chunk);
                    if remaining < width {
                        for ch in chunk.chars() {
                            let width = UnicodeWidthChar::width_cjk(ch).unwrap_or(1);
                            if remaining < width {
                                break 'outer;
                            }

                            queue!(stdout, Print(ch));
                        }
                    } else {
                        queue!(stdout, Print(chunk));
                    }
                }
            }

            queue!(stdout, Clear(ClearType::UntilNewLine));

            // Scroll bar.
            // TODO:
        }

        // Draw the status bar.
        queue!(stdout,
            MoveTo(0, text_height as u16),
            SetBackgroundColor(Color::AnsiValue(250)),
            SetForegroundColor(Color::AnsiValue(233)),
            Print(" "),
            Print(buffer.name()),
            Print(" "),
        );

        if buffer.is_dirty() {
            queue!(stdout,
                SetAttribute(Attribute::Bold),
            SetBackgroundColor(Color::AnsiValue(226)),
                Print("[+]"),
                SetAttribute(Attribute::Reset),
            );
        }

        // Draw the command line.
        // TODO:

        // Draw cursors and selections.

        // Draw the main cursor.
        match buffer.cursors()[0] {
            Cursor::Normal(pos) => {
                queue!(stdout,
                    MoveTo(
                        (pos.x - top_left.x + lineno_width + 1) as u16,
                        (pos.y - top_left.y) as u16
                    ),
                    cursor::Show,
                );
            }
            _ => {
                queue!(stdout, cursor::Hide);
            }
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
