use crate::editor::{EventQueue, Event, Notification, Popup};
use crate::rope::Cursor;
use crate::view::View;
use crate::highlight::Highlighter;
use std::cmp::min;
use std::io::{stdout, Write};
use std::time::Duration;
use std::thread;
pub use crossterm::event::{KeyCode, KeyModifiers, KeyEvent};
use crossterm::event::{self, Event as TermEvent};
use crossterm::{execute, queue};
use crossterm::terminal::{
    size, enable_raw_mode, disable_raw_mode,
    EnterAlternateScreen, LeaveAlternateScreen,
};

fn truncate(s: &str, width: usize) -> &str {
    &s[..min(s.len(), width)]
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

pub struct Terminal {
    rows: usize,
    cols: usize,
}

impl Terminal {
    pub fn new(event_queue: EventQueue) -> Terminal {

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
                                event_queue.enqueue(Event::Key(key));
                            }
                            TermEvent::Mouse(mice) => {
                                trace!("unhandled event: {:?}", mice);
                            }
                            TermEvent::Resize(cols, rows) => {
                                event_queue.enqueue(Event::Resize {
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

    pub fn draw(
        &mut self,
        view: &mut View,
        highlighter: &mut Highlighter,
        notifications: &[Notification],
        popup: &Option<Popup>,
    ) {
        use unicode_width::{UnicodeWidthStr, UnicodeWidthChar};
        use crossterm::cursor::{self, MoveTo};
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
            ).unwrap();
            stdout.flush().unwrap();
            return;
        }

        let lineno_width = num_of_digits(view.buffer().borrow().num_lines()) + 2;
        let text_offset = lineno_width + 2;
        let text_height = self.rows - 2;
        let text_width = self.cols - (3 + lineno_width);
        let status_bar_y = text_height;

        // Adjust top left.
        view.adjust_top_left(text_height, text_width);
        let buffer = view.buffer().borrow();
        let top_left = view.top_left();

        // Highlight the given text.
        let snapshot = buffer.snapshot();
        let modified_line = snapshot.modified_line;
        if top_left.y <= snapshot.modified_line
            && snapshot.modified_line <= top_left.y + text_height {
            let range = modified_line..=modified_line + text_height;
            highlighter.highlight(range, snapshot);
        }

        // Hide the cursor to prevent flickering.
        queue!(stdout,
            cursor::Hide,
        ).unwrap();

        // Draw buffer contents.
        use std::collections::HashMap;
        let mut num_drawed_chars = HashMap::new();
        for i in 0..text_height {
            queue!(stdout, MoveTo(0, i as u16)).unwrap();

            // Line number.
            let y = top_left.y + i;
            let lineno = y + 1; // 1-origin
            let out_of_bounds = lineno > buffer.num_lines();
            if out_of_bounds {
                queue!(stdout,
                    SetBackgroundColor(Color::AnsiValue(240)),
                    Print(whitespaces(lineno_width)),
                    SetAttribute(Attribute::Reset),
                ).unwrap();
            } else {
                queue!(stdout,
                    SetBackgroundColor(Color::AnsiValue(236)),
                    Print(whitespaces(lineno_width - num_of_digits(lineno) - 1)),
                    Print(lineno),
                    Print(" "),
                    SetAttribute(Attribute::Reset),
                ).unwrap();
            }

            // Line map.
            // TODO:
            queue!(stdout,
                SetBackgroundColor(Color::AnsiValue(238)),
                Print(' '),
                SetAttribute(Attribute::Reset),
            ).unwrap();

            // Margin.
            queue!(stdout, Print(' ')).unwrap();

            // Text.
            if !out_of_bounds {
                let mut n = 0;
                let line = buffer.line(y);
                if line.len_chars() > top_left.x {
                    let mut remaining = text_width;
                    let mut spans = highlighter.line_at(y).iter().peekable();
                    let mut current_span = spans.next();
                    let mut next_span = spans.peek();
                    let mut x = top_left.x;
                    let slice = line.slice(top_left.x..);
                    'outer: for mut chunk in slice.chunks() {
                        while remaining > 0 && !chunk.is_empty() {
                            trace!("-------------------");
                            trace!("x={}, range={}", x,
                                current_span.map(|s| *s.range.start()).unwrap_or(99999));
                            match (&current_span, next_span) {
                                (Some(span), _) | (_, Some(span))
                                    if span.range.contains(&x) =>
                                {
                                    queue!(
                                        stdout,
                                        SetAttribute(Attribute::Bold)
                                    ).unwrap();
                                }
                                (Some(_), _) => {
                                    current_span = spans.next();
                                    next_span = spans.peek();
                                    queue!(
                                        stdout,
                                        SetAttribute(Attribute::Reset)
                                    ).unwrap();
                                }
                                (None, _) => {}
                            }

                            let mut num_chars = chunk.chars().count();
                            let mut width = UnicodeWidthStr::width_cjk(chunk);
                            let mut chars_rev = chunk.chars().into_iter().rev();
                            while width > remaining {
                                let ch = chars_rev.next().unwrap();
                                width -= UnicodeWidthChar::width_cjk(ch).unwrap_or(1);
                                num_chars -= 1;
                            }

                            if let Some(span) = current_span {
                                num_chars = min(num_chars, span.range.end() - x);
                            }

                            if let Some(span) = next_span {
                                num_chars = min(num_chars, span.range.start() - x);
                            }

                            num_chars = max(num_chars, 1);
                            let next_ch = chunk.char_indices().skip(num_chars).next();
                            let index =
                                next_ch.map(|(i, _)| i).unwrap_or(chunk.len());

                            queue!(stdout, Print(&chunk[..index])).unwrap();

                            chunk = &chunk[min(index, chunk.len())..];
                            remaining -= width;
                            x += num_chars;
                            n += num_chars;
                        }
                    }
                }

                num_drawed_chars.insert(y, n);
            }

            queue!(stdout, Clear(ClearType::UntilNewLine)).unwrap();

            // Scroll bar.
            // TODO:
        }

        // Draw the status bar.
        queue!(stdout,
            MoveTo(0, status_bar_y as u16),
            SetBackgroundColor(Color::AnsiValue(250)),
            SetForegroundColor(Color::AnsiValue(233)),
            Print(" "),
            SetAttribute(Attribute::Bold),
            SetAttribute(Attribute::Underlined),
            Print(buffer.name()),
            SetAttribute(Attribute::NoUnderline),
            Print(" "),
            SetAttribute(Attribute::Reset),
        ).unwrap();

        if buffer.is_dirty() {
            queue!(stdout,
                SetAttribute(Attribute::Bold),
                SetBackgroundColor(Color::AnsiValue(226)),
                Print("[+]"),
                SetAttribute(Attribute::Reset),
            ).unwrap();
        }

        // Draw the notification.
        let mut iter = notifications.iter().rev();
        if let Some(noti) = iter.next() {
            queue!(stdout, MoveTo(0, status_bar_y as u16 + 1)).unwrap();
            if noti.created_at.elapsed() < Duration::from_secs(3) {
                let num_duplicated = iter.take_while(|x| *x == noti).count() + 1;
                if num_duplicated > 1 {
                    queue!(
                        stdout,
                        Print(&format!("({}) {}",
                        num_duplicated,
                            truncate(&noti.message,
                                self.cols - 3 - num_of_digits(num_duplicated))
                            )
                    )).unwrap();
                } else {
                    queue!(stdout, Print(truncate(&noti.message, self.cols))).unwrap();
                }
            }

            queue!(stdout, Clear(ClearType::UntilNewLine)).unwrap();
        }

        // Draw popup.
        let main_cursor = &buffer.cursors()[0];
        if buffer.cursors().len() == 1 {
            if let(Some(popup), Cursor::Normal { pos, .. }) = (popup, main_cursor) {
                let cursor_y = pos.y - top_left.y;
                let cursor_x = pos.x - top_left.x;
                let longest =
                    popup.lines.iter().map(String::len).max().unwrap_or(0);
                let popup_width = min(longest + 1, text_width - 3);
                let x = if cursor_x + popup_width < text_width {
                    cursor_x
                } else {
                    text_width - popup_width
                };

                let (y, popup_height) = if cursor_y + 1 + popup.lines.len() < text_height {
                    (cursor_y + 1, popup.lines.len())
                } else {
                    (cursor_y + 1, text_height - cursor_y - 1)
                };

                for i in 0..popup_height {
                    let item = &popup.lines[i];
                    queue!(
                        stdout,
                        MoveTo((text_offset + x) as u16, (y + i) as u16),
                        SetBackgroundColor(Color::AnsiValue(89)),
                        SetAttribute(Attribute::Bold),
                        Print(truncate(&item, popup_width - 1)),
                        Print(whitespaces(popup_width - item.len())),
                        SetAttribute(Attribute::Reset),
                    ).unwrap();
                }
            }
        }

        // Draw cursors and selections.
        for (i, c) in buffer.cursors().iter().enumerate() {
            match c {
                Cursor::Normal { .. } if i == 0 => {
                    // Do nothing: we use the "real" cursor for it.
                }
                Cursor::Normal { pos, .. } => {
                    // Draw a "fake" cursor.
                    if pos.y < top_left.y + text_height
                        && pos.x - top_left.x < num_drawed_chars[&pos.y]
                    {
                        queue!(stdout,
                            MoveTo(
                                (pos.x - top_left.x + text_offset) as u16,
                                (pos.y - top_left.y) as u16,
                            ),
                            SetAttribute(Attribute::Reverse),
                            Print(buffer.line(pos.y).char(pos.x)),
                            SetAttribute(Attribute::NoReverse)
                        ).unwrap();
                    }
                }
                Cursor::Selection(range) => {
                    let mut pos = *range.front();
                    let end = range.back();
                    if pos.y < top_left.y || pos.y >= buffer.num_lines() {
                        continue;
                    }

                    queue!(stdout, SetAttribute(Attribute::Reverse)).unwrap();
                    while pos != *end
                        && pos.y < top_left.y + text_height
                        && pos.x - top_left.x < num_drawed_chars[&pos.y]
                    {
                        if pos.x >= buffer.line_len(pos.y) {
                            pos.y += 1;
                            pos.x = 0;
                        } else {
                            queue!(stdout,
                                MoveTo(
                                    (pos.x - top_left.x + text_offset) as u16,
                                    (pos.y - top_left.y) as u16,
                                ),
                                Print(buffer.line(pos.y).char(pos.x)),
                            ).unwrap();
                            pos.x += 1;
                        }
                    }

                    queue!(stdout, SetAttribute(Attribute::NoReverse)).unwrap();
                }
            }
        }

        // Draw the main cursor.
        match main_cursor {
            Cursor::Normal { pos, .. } => {
                queue!(stdout,
                    MoveTo(
                        (pos.x - top_left.x + text_offset) as u16,
                        (pos.y - top_left.y) as u16
                    ),
                    cursor::Show,
                ).unwrap();
            }
            _ => {
                queue!(stdout, cursor::Hide).unwrap();
            }
        }
        stdout.flush().unwrap();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen).unwrap();
        disable_raw_mode().unwrap();
    }
}
