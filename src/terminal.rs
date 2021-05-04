use std::cmp::min;
use std::io::Write;
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
    buffer::Buffer,
    eventloop::{Event, EventQueue},
    rope::Point,
    view::{Span, View},
};

pub struct DrawContext<'a> {
    pub buffer: &'a Buffer,
    pub view: &'a mut View,
}

pub struct Terminal {
    screen_height: usize,
    screen_width: usize,
}

fn whitespaces(n: usize) -> String {
    " ".repeat(n)
}

pub fn truncate(s: &str, width: usize) -> &str {
    &s[..min(s.chars().count(), width)]
}

pub trait DisplayWidth {
    fn display_width(&self) -> usize;
}

impl DisplayWidth for str {
    fn display_width(&self) -> usize {
        unicode_width::UnicodeWidthStr::width_cjk(self)
    }
}

impl DisplayWidth for char {
    fn display_width(&self) -> usize {
        unicode_width::UnicodeWidthChar::width_cjk(*self).unwrap_or(1)
    }
}

impl DisplayWidth for usize {
    fn display_width(&self) -> usize {
        let mut n = *self;
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
}

impl Terminal {
    pub fn new(event_queue: EventQueue) -> Terminal {
        let (cols, rows) = size().expect("failed to get the terminal size");
        enable_raw_mode().expect("failed to enable the raw mode");
        execute!(stdout(), EnterAlternateScreen).expect("failed to enter the alternative screen");

        tokio::spawn(async move {
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
        });

        Terminal {
            screen_height: rows as usize,
            screen_width: cols as usize,
        }
    }

    pub fn draw(&mut self, ctx: DrawContext) {
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

        let lineno_width = ctx.buffer.num_lines().display_width() + 1;
        let text_max_height = self.screen_height - 1;
        let text_max_width = self.screen_width - lineno_width;

        ctx.view
            .layout(&ctx.buffer, 0, text_max_width, text_max_height);

        for (i, display_line) in ctx.view.visible_display_lines().iter().enumerate() {
            queue!(stdout, cursor::MoveTo(0, i as u16)).ok();

            // Draw the line number.
            let lineno = display_line.range.front().y + 1;
            queue!(
                stdout,
                Print(whitespaces(lineno_width - lineno.display_width() - 1)),
                Print(lineno),
                Print('\u{2502}' /* "Box Drawing Light Veritical" */),
                SetAttribute(Attribute::Reset),
            )
            .ok();

            // Draw buffer contents.
            let rope_line = ctx.buffer.line(lineno - 1);
            for span in &display_line.spans {
                match span {
                    Span::Text { char_range } => {
                        queue!(stdout, Print(rope_line.slice(char_range.clone()))).ok();
                    }
                    Span::Style(style) => {
                        // TODO:
                    }
                }
            }

            queue!(stdout, Clear(ClearType::UntilNewLine));
        }

        let main_cursor = ctx.buffer.main_cursor_pos();
        let column = main_cursor.x + 1;
        let buffer_name = truncate(ctx.buffer.name(), self.screen_width.saturating_sub(16));
        let status_line_width = buffer_name.display_width() + 1 + column.display_width();
        queue!(
            stdout,
            MoveTo(
                (self.screen_width - status_line_width) as u16,
                text_max_height as u16
            ),
            SetForegroundColor(if ctx.buffer.is_dirty() {
                Color::Yellow
            } else {
                Color::Reset
            }),
            Print(buffer_name),
            Print(' '),
            Print(column),
            MoveTo(
                (lineno_width + main_cursor.x) as u16,
                (main_cursor.y) as u16,
            ),
            cursor::Show,
        )
        .ok();

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
