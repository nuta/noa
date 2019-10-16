use termion;
use termion::input::TermRead;
use termion::event::{Key, Event as TEvent};
use termion::raw::{IntoRawMode, RawTerminal};
use signal_hook::{self, iterator::Signals};
use std::cmp::max;
use std::fmt::Write;
use std::sync::mpsc::Sender;
use crate::frontend::{FrontEnd, Event};
use crate::screen::{Screen, RectSize, Panel, View, Mode};
use crate::buffer::Buffer;
use crate::file::File;

pub struct Terminal {
    stdout: RawTerminal<std::io::Stdout>,
    buf: String,
}

impl Terminal {
    pub fn new() -> Terminal {
        let stdout = std::io::stdout().into_raw_mode().unwrap();
        Terminal {
            stdout,
            buf: String::with_capacity(200 * 80 * 4 /* Assume a big screen. */),
        }
    }

    fn draw_buffer(&mut self, panel: &Panel, view: &View, file: &File, buffer: &Buffer) {
        for i in 0..(panel.height() - 2) {
            let lineno = view.top_left().line + i;
            if lineno >= buffer.num_lines() {
                break;
            }

            let y = panel.top_left().line + i;
            let x = panel.top_left().column;
            write!(self.buf, "{}", goto(y, x)).ok();
            let highlighted_spans =
                file.highlight(lineno, 0 /* TODO: */, panel.width());
            for (style, text) in highlighted_spans {
                if let Some(style) = style {
                    let c = style.foreground;
                    let seq = 
                        termion::color::Fg(termion::color::Rgb(c.r, c.g, c.b));
                    write!(self.buf, "{}", seq).ok();
                }

                write!(self.buf, "{}", text).ok();
            }
        }

        write!(self.buf, "{}", termion::style::Reset).ok();
    }

    fn draw_status_bar(&mut self, panel: &Panel, view: &View, buffer: &Buffer) {
        let cursor = view.cursor();
        let status_bar_len =
            6 + buffer.name().len() + num_of_digits(cursor.column + 1);

        if panel.width() < status_bar_len {
            warn!("too small to draw the status bar!");
            return;
        }

        let space_len = panel.width() - status_bar_len;
        write!(
            self.buf,
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
            goto = goto(panel.height() - 2, panel.top_left().column),
            invert = termion::style::Invert,
            bold = termion::style::Bold,
            name = buffer.name(),
            modified = if buffer.modified() { " [+]" } else { "    " },
            nobold = termion::style::NoBold,
            space_len = space_len,
            column = cursor.column + 1,
            reset = termion::style::Reset,
        ).ok();
    }

    fn draw_cursor(&mut self, screen: &Screen) {
        // TODO: Support lines which contains wide width characters.

        let active_panel = screen.current_panel();
        let active_view = screen.active_view();
        let cursor = active_view.cursor();
        let top_left = active_panel.top_left();
        let cursor_y = top_left.line + cursor.line;
        let cursor_x = top_left.column + cursor.column;
        write!(self.buf, "{}", goto(cursor_y, cursor_x)).ok();
    }

    fn draw_finder(&mut self, screen: &Screen) {
        if screen.mode() == Mode::Finder {
            // Hard-coded preferences.
            let menu_width = 50;
            let margin_top = 2;
            let margin_bottom = 2;
            let menu_height_max = 20;

            if screen.height() < margin_top + margin_bottom + 2
                || screen.width() < menu_width {
                warn!("too small screen to show the command menu!");
                return;
            }

            let finder = screen.finder();
            let menu_height = max(
                screen.height() - (margin_top + margin_bottom),
                menu_height_max
            );
            let x = screen.width() / 2 - menu_width / 2;

            // Input box.
            write!(
                self.buf,
                "{goto}{color}{text:<menu_width$}{reset}",
                goto = goto(margin_top, x),
                color = termion::color::Bg(termion::color::Cyan),
                text = finder.textbox().text(),
                menu_width = menu_width,
                reset = termion::color::Bg(termion::color::Reset)
            ).ok();

            // Results.
            let results = finder.filtered().iter().enumerate()
                              .take(menu_height - 1);
            for (i, cmd) in results {
                write!(
                    self.buf,
                    "{goto}{color}{selected}{title:<menu_width$}{reset}",
                    goto = goto(margin_top + 1 + i, x),
                    color = termion::color::Bg(termion::color::Magenta),
                    selected = if i == finder.selected() { "> " } else { "  "},
                    title = cmd.title,
                    menu_width = menu_width - 2,
                    reset = termion::color::Bg(termion::color::Reset)
                ).ok();
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        use std::io::Write;
        write!(self.stdout, "{}", termion::screen::ToMainScreen).ok();
        self.stdout.flush().ok();
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
    fn init(&mut self, event_queue: Sender<Event>) {
        use std::io::Write;
        write!(self.stdout, "{}", termion::screen::ToAlternateScreen).ok();

        // Read keyboard inputs in a dedicated thread.
        let stdin_tx = event_queue.clone();
        std::thread::spawn(move || {
            let mut stdin = std::io::stdin().events();
            loop {
                let raw_event = stdin.next().unwrap().unwrap();
                let event = match raw_event {
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
                            Key::Esc => Event::Esc,
                            _ => Event::Unknown,
                        }
                    }
                    TEvent::Unsupported(seq) => {
                        warn!("unsupported key sequence: {:?}", seq);
                        Event::Unknown
                    }
                    _ => {
                        warn!("unsupported input event: {:?}", raw_event);
                        Event::Unknown
                    }
                };

                stdin_tx.send(event).ok();
            }
        });

        // Wait for signals in a dedicated thread.
        let signal_tx = event_queue.clone();
        std::thread::spawn(move || {
            let signals = Signals::new(&[signal_hook::SIGWINCH]).unwrap();
            for signal in &signals {
                match signal {
                    signal_hook::SIGWINCH => {
                        signal_tx.send(Event::ScreenResized).ok();
                        info!("received SIGWINCH");
                    },
                    _ => {
                        warn!("unhandled signal: {}", signal);
                    },
                }
            }
        });
    }

    fn render(&mut self, screen: &Screen) {
        self.buf.clear();

        // Clear the entire screen.
        write!(self.buf, "{}{}", termion::clear::All, termion::cursor::Hide).ok();

        for panel in screen.panels() {
            let view = panel.view();
            let file = view.file();
            let buffer = file.buffer();

            if panel.height() < 2 {
                warn!("too small panel!");
                return;
            }

            self.draw_buffer(panel, view, &*file, &*buffer);
            self.draw_status_bar(panel, view, &*buffer);
        }

        self.draw_cursor(screen);
        self.draw_finder(screen);

        write!(self.buf, "{}", termion::cursor::Show).ok();

        use std::io::Write;
        self.stdout.write_all(self.buf.as_bytes()).unwrap();
        self.stdout.flush().unwrap();
    }


    fn get_screen_size(&self) -> RectSize {
        let size = termion::terminal_size().unwrap();
        RectSize {
            height: size.1 as usize,
            width: size.0 as usize,
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