use crate::buffer::{Buffer, Line};
use crate::editor::Event;
use std::cmp::min;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use termion::event::Event as TermEvent;
pub use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::AlternateScreen;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Rgb(u32);
impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Rgb {
        Rgb((r as u32) << 16 | (g as u32) << 8 | (b as u32))
    }

    pub fn r(self) -> u8 {
        (self.0 >> 16) as u8
    }

    pub fn g(self) -> u8 {
        ((self.0 >> 8) & 0xff) as u8
    }

    pub fn b(self) -> u8 {
        (self.0 & 0xff) as u8
    }

    pub fn as_term_rgb(self) -> termion::color::Rgb {
        termion::color::Rgb(self.r(), self.g(), self.b())
    }
}

pub struct PromptItem {
    pub title: String,
    pub label: char,
    pub color: Rgb,
}

impl PromptItem {
    pub const PATH_COLOR: Rgb = Rgb::new(0, 100, 100);
    pub const BUFFER_COLOR: Rgb = Rgb::new(50, 50, 140);
    pub const UNSAVED_BUFFER_COLOR: Rgb = Rgb::new(200, 50, 50);
    pub fn new(label: char, color: Rgb, title: String) -> PromptItem {
        PromptItem {
            label,
            title,
            color,
        }
    }
}

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

fn display_width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}

fn truncate(s: &str, width: usize) -> &str {
    &s[..min(s.len(), width)]
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
    stdout: AlternateScreen<RawTerminal<std::io::Stdout>>,
    tx: Sender<Event>,
    width: usize,
    height: usize,
}

impl Terminal {
    pub fn new(tx: Sender<Event>) -> Terminal {
        let mut stdout = AlternateScreen::from(std::io::stdout().into_raw_mode().unwrap());

        // Clear the screen.
        use std::io::Write;
        write!(stdout, "{}", termion::clear::All).ok();

        // Read inputs.
        let tx1 = tx.clone();
        std::thread::spawn(move || {
            let mut stdin = std::io::stdin().events();
            loop {
                if let Some(ev) = stdin.next() {
                    match ev {
                        Ok(ev) => match ev {
                            TermEvent::Key(key) => {
                                tx1.send(Event::Key(key)).ok();
                            }
                            _ => {}
                        },
                        Err(_) => { /* ignore errors */ }
                    }
                }
            }
        });

        let size = termion::terminal_size().unwrap();
        Terminal {
            stdout,
            tx,
            width: size.0 as usize,
            height: size.1 as usize,
        }
    }

    pub fn text_height(&self) -> usize {
        self.height - 2
    }

    pub fn update_screen_size(&mut self) {
        let size = termion::terminal_size().unwrap();
        self.width = size.0 as usize;
        self.height = size.1 as usize;
    }

    pub fn render_editor<'a>(
        &mut self,
        buffer: &mut Buffer,
        message: Option<&str>,
        statuses: impl Iterator<Item = &'a (String, Rgb)>,
    ) {
        use std::io::Write;
        use termion::{clear, color, cursor, style};

        if self.width < 10 || self.height < 10 {
            warn!("screen is too small!");
            return;
        }

        // Hide the cursor to mitigate flickering.
        write!(self.stdout, "{}", cursor::Hide).ok();

        // Adjust y-axis first to compute lineno_width.
        let mut height = self.height - 2;
        buffer.adjust_top_left(height, self.width);

        // Adjust x-axis.
        let mut lineno = buffer.top_left().y + 1;
        let lineno_width = num_of_digits(lineno + height);
        let text_width = self.width - lineno_width - 2;
        buffer.adjust_top_left(height, text_width);

        // Now we have the correct top_left.
        let top_left = buffer.top_left();

        // Buffer contents.
        let mut display_x = 0;
        let cursor = buffer.cursor();
        let lines = buffer.lines().skip(top_left.y).take(height);
        for (y, line) in lines.enumerate() {
            write!(
                self.stdout,
                "{} {}{} ",
                cursor::Goto(1, 1 + y as u16),
                whitespaces(lineno_width - num_of_digits(lineno)),
                lineno
            )
            .ok();

            let mut remaining = text_width;
            let tab_width = buffer.config().tab_width;
            for (x, ch) in line.substr(top_left.x, text_width).chars().enumerate() {
                let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
                if remaining < ch_width {
                    break;
                }

                if ch == '\t' {
                    let mut n = min(remaining, tab_width - (x % tab_width));
                    if n == 0 {
                        n = tab_width;
                    }

                    write!(self.stdout, "{}", whitespaces(n)).ok();
                    if top_left.y + y == cursor.y && top_left.x + x < cursor.x {
                        display_x += n;
                    }
                } else {
                    write!(self.stdout, "{}", ch).ok();
                    if top_left.y + y == cursor.y && top_left.x + x < cursor.x {
                        display_x += 1;
                    }
                }
                remaining -= ch_width;
            }

            write!(self.stdout, "{}", clear::UntilNewline).ok();
            lineno += 1;
            height -= 1;
        }

        // Clear remaining lines.
        while height > 0 {
            write!(self.stdout, "{}{}", cursor::Down(1), clear::CurrentLine,).ok();
            height -= 1;
        }

        // The status bar.
        let filename = truncate(buffer.display_name(), self.width.saturating_sub(10));
        write!(
            self.stdout,
            "{}{}{} {} | {}{} {}",
            cursor::Goto(1, 1 + (self.height - 2) as u16),
            style::Invert,
            style::Bold,
            filename,
            style::NoBold,
            buffer.cursor().x + 1,
            style::Reset,
        )
        .ok();

        // Statuses.
        let mut remaining_width =
            self.width - (display_width(filename) + num_of_digits(buffer.cursor().x + 1) + 5);
        for (status, rgb) in statuses {
            if status.len() + 2 <= remaining_width {
                write!(self.stdout, "{} {} ", color::Bg(rgb.as_term_rgb()), status).ok();
                remaining_width -= status.len() + 2;
            }
        }

        write!(self.stdout, "{}{}", style::Reset, clear::UntilNewline).ok();

        // The message line.
        write!(
            self.stdout,
            "{}{}{}",
            cursor::Goto(1, 1 + (self.height - 1) as u16),
            truncate(message.unwrap_or(""), self.width),
            clear::UntilNewline,
        )
        .ok();

        // Move the cursor to the correct position and show it.
        let x = lineno_width + 2 + display_x;
        let y = cursor.y - top_left.y;
        write!(
            self.stdout,
            "{}{}",
            cursor::Goto(1 + x as u16, 1 + y as u16),
            cursor::Show,
        )
        .ok();

        self.stdout.flush().ok();
    }

    pub fn render_prompt(
        &mut self,
        title: &str,
        user_input: &Line,
        cursor: usize,
        selected: usize,
        items: &[Rc<PromptItem>],
    ) {
        use std::io::Write;
        use termion::{clear, color, cursor::Goto, style};
        write!(
            self.stdout,
            "{}{} {} {}{}{}",
            Goto(1, 1),
            style::Bold,
            title,
            style::Reset,
            user_input.substr(0 /* TODO: */, self.width - title.len() - 4),
            clear::UntilNewline,
        )
        .ok();

        let title_width = self.width - 4;
        let mut remaining_height = min(32, self.height.saturating_sub(5));
        for (i, item) in items.iter().enumerate().take(remaining_height) {
            if i == selected {
                write!(
                    self.stdout,
                    "\r\n{}{} {} {} {}{}{}{}",
                    style::Bold,
                    color::Bg(item.color.as_term_rgb()),
                    item.label,
                    color::Bg(color::Reset),
                    style::Underline,
                    truncate(&item.title, title_width),
                    style::Reset,
                    clear::UntilNewline,
                )
                .ok();
            } else {
                write!(
                    self.stdout,
                    "\r\n{} {} {} {}{}",
                    color::Bg(item.color.as_term_rgb()),
                    item.label,
                    style::Reset,
                    truncate(&item.title, title_width),
                    clear::UntilNewline,
                )
                .ok();
            }
            remaining_height -= 1;
        }

        for _ in 0..remaining_height {
            write!(self.stdout, "\r\n{}", clear::UntilNewline).ok();
        }

        write!(
            self.stdout,
            "{}",
            Goto(1 + 2 + (cursor + title.len()) as u16, 1)
        )
        .ok();
        self.stdout.flush().ok();
    }
}
