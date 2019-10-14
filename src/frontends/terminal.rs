use termion;
use termion::input::TermRead;
use termion::event::{Key, Event as TEvent};
use termion::raw::{IntoRawMode, RawTerminal};
use std::io::Write;
use crate::frontend::{FrontEnd, Event, ScreenSize};
use crate::layout::Layout;

pub struct Terminal {
    stdin: termion::input::Events<std::io::Stdin>,
    stdout: RawTerminal<std::io::Stdout>,
}

impl Terminal {
    pub fn new() -> Terminal {
        let stdin = std::io::stdin().events();
        let mut stdout = std::io::stdout().into_raw_mode().unwrap();
        write!(stdout, "{}", termion::screen::ToAlternateScreen).unwrap();
        Terminal {
            stdin,
            stdout,
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        write!(self.stdout, "{}", termion::screen::ToMainScreen).unwrap();
        self.stdout.flush().unwrap();
    }
}

fn goto(y: usize, x: usize) -> termion::cursor::Goto {
    termion::cursor::Goto(1 + x as u16, 1 + y as u16)
}

fn num_of_digits(mut x: usize) -> usize {
    let mut num = 1;
    let base = 10;
    while x >= base {
        x /= base;
        num += 1;
    }
    num
}

impl FrontEnd for Terminal {
    fn render(&mut self, layout: &Layout) {
        // Clear the entire screen.
        write!(self.stdout, "{}", termion::clear::All).unwrap();

        // Fill each panels.
        for panel in layout.panels() {
            let view = panel.current_view();
            let file = view.file();
            let buffer = file.buffer();

            if panel.height() < 2 {
                warn!("too small panel!");
                return;
            }

            // Fill the buffer text.
            for i in 0..(panel.height() - 2) {
                let lineno = view.top_left().line + i;
                if lineno >= buffer.num_lines() {
                    break;
                }

                // FIXME: Avoid constructing a temporary string.
                let line: String = buffer.line_at(lineno).collect();
                let y = panel.top_left().line + i;
                let x = panel.top_left().column;
                write!(self.stdout, "{}{}", goto(y, x), line)
                    .unwrap();
            }
        
            // Draw the status bar.
            let cursor = view.cursor();
            let status_bar_len =
                6 + buffer.name().len() + num_of_digits(cursor.column + 1);
            let space_len = panel.width() - status_bar_len;
            write!(
                self.stdout,
                concat!(
                    "{goto}{invert}",
                     " ",
                    "{bold}{name}{modified}{nobold}",
                    "{:space_len$}",
                    "{column}",
                    " ",
                    "{reset}"
                ),
                ' ',
                goto = goto(panel.height() - 2, 0),
                invert = termion::style::Invert,
                bold = termion::style::Bold,
                name = buffer.name(),
                modified = if buffer.modified() { " [+]" } else { "    " },
                nobold = termion::style::NoBold,
                space_len = space_len,
                column = cursor.column + 1,
                reset = termion::style::Reset,
            ).unwrap();
        }

        // Move the cursor.
        let active_panel = layout.current_panel();
        let active_view = layout.active_view();
        let cursor = active_view.cursor();
        let top_left = active_panel.top_left();
        let cursor_y = top_left.line + cursor.line;
        let cursor_x = top_left.column + cursor.column;
        write!(self.stdout, "{}", goto(cursor_y, cursor_x)).unwrap();

        // Update the screen.
        self.stdout.flush().unwrap();
    }

    fn get_screen_size(&self) -> ScreenSize {
        let size = termion::terminal_size().unwrap();
        ScreenSize {
            height: size.1 as usize,
            width: size.0 as usize,
        }
    }

    fn read_event(&mut self) -> Event {
        let event = self.stdin.next().unwrap().unwrap();
        match event {
            TEvent::Key(key) => {
                match key {
                    Key::Alt(ch) => Event::Alt(ch),
                    Key::Ctrl(ch) => Event::Ctrl(ch),
                    Key::Char(ch) => Event::Char(ch),
                    Key::Up => Event::Up,
                    Key::Down => Event::Down,
                    Key::Left => Event::Left,
                    Key::Right => Event::Right,
                    Key::Backspace => Event::Backspace,
                    Key::Delete => Event::Delete,
                    _ => Event::Unknown,
                }
            }
            TEvent::Unsupported(seq) => {
                warn!("unsupported key sequence: {:?}", seq);
                Event::Unknown
            }
            _ => {
                warn!("unsupported input event: {:?}", event);
                Event::Unknown
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_of_digits() {
        assert_eq!(num_of_digits(0), 1);
        assert_eq!(num_of_digits(1), 1);
        assert_eq!(num_of_digits(10), 2);
        assert_eq!(num_of_digits(99), 2);
        assert_eq!(num_of_digits(100), 3);
    }
}
