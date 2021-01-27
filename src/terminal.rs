use crate::buffer::{Buffer, TopLeft};
use crate::editor::{Event, Notification, NotificationLevel};
use crate::finder::{Finder, FinderItem};
use crate::line_edit::LineEdit;
use crate::rope::{Cursor, Point};
use crossterm::cursor::{self, MoveTo};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event as TermEvent};
pub use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent as RawMouseEvent, MouseEventKind,
};
use crossterm::style::{
    Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::terminal::{Clear, ClearType};
use crossterm::{execute, queue};
use std::cmp::{max, min};
use std::collections::HashMap;
use std::io::{stdout, Write};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

pub trait DisplayWidth {
    fn display_width(&self) -> usize;
}

impl DisplayWidth for char {
    fn display_width(&self) -> usize {
        unicode_width::UnicodeWidthChar::width_cjk(*self).unwrap_or(1)
    }
}

impl DisplayWidth for str {
    fn display_width(&self) -> usize {
        unicode_width::UnicodeWidthStr::width_cjk(self)
    }
}

pub fn truncate(s: &str, width: usize) -> &str {
    &s[..min(s.chars().count(), width)]
}

pub fn with_ellipsis(s: &str, width: usize) -> String {
    if s.display_width() <= width {
        return s.to_owned();
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let left_width = (width - 3) / 2;
    let mut left_end = 0;
    for (i, _) in s.char_indices() {
        if s[..i].display_width() >= left_width {
            // Don't update left_end to use the previous character.
            left_end = i;
            break;
        }
    }

    let right_width = width - 3 - left_width;
    let mut right_start = s.len();
    for (i, _) in s.char_indices().rev() {
        if s[i..].display_width() >= right_width {
            right_start = i;
            break;
        }
    }

    let mut new_s = String::with_capacity(width);
    new_s.push_str(&s[..left_end]);
    new_s.push_str("...");
    new_s.push_str(&s[right_start..]);
    new_s
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

pub fn handle_term_event(event_queue: Sender<Event>) {
    fn handle_event(event_queue: &Sender<Event>, ev: TermEvent) {
        match ev {
            TermEvent::Key(key) => {
                event_queue.send(Event::Key(key)).ok();
            }
            TermEvent::Mouse(mouse) => {
                event_queue.send(Event::Mouse(mouse)).ok();
            }
            TermEvent::Resize(cols, rows) => {
                event_queue
                    .send(Event::Resize {
                        cols: cols as usize,
                        rows: rows as usize,
                    })
                    .ok();
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

                    event_queue.send(Event::KeyBatch(buf)).ok();
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
}

#[derive(Debug)]
pub enum MouseEvent {
    ClickText {
        /// The position in the buffer. They could be out of bounds of the text.
        pos: Point,
        alt: bool,
    },
    DoubleClickText {
        /// The position in the buffer. They could be out of bounds of the text.
        pos: Point,
        alt: bool,
    },
    ClickLineNo {
        y: usize,
    },
    DragLineNo {
        y: usize,
    },
    DragText {
        /// The position in the buffer. They could be out of bounds of the text.
        pos: Point,
    },
    HoverText {
        /// The position in the buffer. They could be out of bounds of the text.
        pos: Point,
    },
    ScrollUp,
    ScrollDown,
}

/// Represents each character cell in the terminal (not RGB pixel).
#[derive(Debug, PartialEq, Eq, Clone)]
struct Pixel {
    y: usize,
    x: usize,
}

impl Pixel {
    pub const fn new(y: usize, x: usize) -> Pixel {
        Pixel { y, x }
    }
}

pub struct PixelMap {
    map: Vec<Option<Point>>,
    width: usize,
}

impl PixelMap {
    pub fn new() -> PixelMap {
        PixelMap {
            map: Vec::new(),
            width: 0,
        }
    }

    pub fn clear(&mut self, rows: usize, cols: usize) {
        self.width = cols;
        self.map.resize(rows * cols, None);
        self.map.fill(None);
    }

    pub fn set_pixel(&mut self, display_pos: &Point, text_pos: &Point) {
        let index = self.display_pos_to_index(display_pos);
        self.map[index] = Some(text_pos.clone());
    }

    pub fn display_pos_to_text_pos(&self, display_pos: &Point) -> Option<&Point> {
        trace!(
            "pixel: {} -> {:?}",
            display_pos,
            self.map.get(self.display_pos_to_index(display_pos))
        );
        self.map
            .get(self.display_pos_to_index(display_pos))
            .map(|o| o.as_ref())
            .unwrap_or(None)
    }

    fn display_pos_to_index(&self, display_pos: &Point) -> usize {
        display_pos.y * self.width + display_pos.x
    }
}

pub struct Terminal {
    rows: usize,
    cols: usize,
    last_clicked: Option<Instant>,
    pixel_map: PixelMap,

    // TODO: Remove,
    text_cols: usize,
    current_top_left: TopLeft,
    display_x_text_start: usize,
}

impl Terminal {
    pub fn new(event_queue: Sender<Event>) -> Terminal {
        let (cols, rows) = size().expect("failed to get the terminal size");
        enable_raw_mode().expect("failed to enable the raw mode");
        queue!(
            stdout(),
            EnterAlternateScreen,
            Clear(ClearType::All),
            EnableMouseCapture,
        )
        .ok();

        thread::spawn(move || {
            handle_term_event(event_queue);
        });

        Terminal {
            rows: rows as usize,
            cols: cols as usize,
            text_cols: 0, // Filled in draw.
            current_top_left: TopLeft::new(0, 0),
            last_clicked: None,
            display_x_text_start: 0,
            pixel_map: PixelMap::new(),
        }
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn text_cols(&self) -> usize {
        self.text_cols
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.rows = rows;
        self.cols = cols;
    }

    pub fn draw_buffer2(
        &mut self,
        buffer: &mut Buffer,
        notification: Option<&Notification>,
        cursor_hover: Option<&Point>,
    ) {
        let mut stdout = stdout();
        if self.cols < 20 || self.rows < 5 {
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

        queue!(stdout, cursor::Hide,).ok();

        self.pixel_map.clear(self.rows, self.cols);

        trace!("self.rows={}, self.cols={}", self.rows, self.cols);
        let lineno_width = num_of_digits(buffer.num_lines()) + 1;
        let display_x_text_start = lineno_width;
        let text_height = self.rows - 1;
        let text_width = self.cols - display_x_text_start;
        self.text_cols = text_width;

        // Adjust top left.
        buffer.adjust_top_left(text_height, text_width);
        let top_left = buffer.top_left();
        self.current_top_left = top_left.clone();
        self.display_x_text_start = display_x_text_start;

        let main_pos = buffer.main_cursor_pos();

        use ropey::iter::Chunks;
        use std::iter::Peekable;

        // Draw the text area.
        let mut y = top_left.y;
        let mut wrapped: Option<Peekable<Chunks>> = None;
        let mut cursor_pos = None;
        let mut in_selection = false;
        let mut x_base = top_left.x;
        let mut wrapped = if top_left.x > 0 {
            let mut chunks = buffer.line(y).chunks().peekable();
            let (chunk_i, char_i) = buffer.line_substr_chunk(y, top_left.x).unwrap();
            for chunk in 0..chunk_i {
                chunks.next();
            }
            Some((chunks, char_i))
        } else {
            None
        };

        for display_y in 0..text_height {
            // Move the cursor at the beginning of the next line.
            queue!(
                stdout,
                MoveTo(0, display_y as u16 + 1),
                SetAttribute(Attribute::Reset),
            )
            .ok();

            // Handle the cursor at the end of file.
            if y == main_pos.y && main_pos.y == buffer.num_lines() && cursor_pos.is_none() {
                cursor_pos = Some(Point::new(display_y, 0));
            }

            // Get the string chunks of the current (or next) line.
            let is_wrapped = wrapped.is_some();
            let (mut chunks, mut chunk_char_start) = match wrapped {
                Some(inner) => inner,
                None if y < buffer.num_lines() => {
                    x_base = 0;
                    (buffer.line(y).chunks().peekable(), 0)
                }
                None => {
                    // Out of bounds.
                    trace!(
                        "skip: display_y={}/{}, y={}/{}",
                        display_y,
                        text_height,
                        y,
                        buffer.num_lines()
                    );
                    queue!(
                        stdout,
                        SetAttribute(Attribute::Reset),
                        Clear(ClearType::UntilNewLine),
                    )
                    .ok();
                    continue;
                }
            };

            trace!(
                "display_y={}/{}, x={}/{}, chunk_start={:?}",
                display_y,
                text_height,
                x_base,
                buffer.line_len(y),
                chunk_char_start
            );

            // Line number.
            if is_wrapped {
                queue!(
                    stdout,
                    SetForegroundColor(Color::DarkGrey),
                    Print(whitespaces(lineno_width - 2)),
                    Print("~"),
                    SetAttribute(Attribute::Reset),
                    Print(' '),
                )
                .ok();
            } else {
                if y == main_pos.y {
                    queue!(stdout, SetAttribute(Attribute::Bold)).ok();
                } else {
                    queue!(stdout, SetForegroundColor(Color::DarkGrey)).ok();
                }

                queue!(
                    stdout,
                    Print(whitespaces(lineno_width - num_of_digits(y + 1) - 1)),
                    Print(y + 1),
                    Print(' '),
                    SetAttribute(Attribute::Reset),
                )
                .ok();
            }

            if let Cursor::Selection { range, .. } = &buffer.cursor() {
                if in_selection && *range.back() == Point::new(y, 0) {
                    in_selection = false;
                } else if in_selection || *range.front() == Point::new(y, 0) {
                    in_selection = true;
                    queue!(stdout, SetAttribute(Attribute::Reverse)).ok();
                    if buffer.line_len(y) == 0 {
                        queue!(stdout, Print(' ')).ok();
                    }
                }
            }

            // Text.
            let mut chunk_i = 0;
            let mut display_x = 0;
            let mut remaining_width = text_width;
            let mut end_x = 0;
            'outer: while remaining_width > 0 {
                let s = match chunks.peek() {
                    Some(s) => s,
                    None => break,
                };

                trace!("next chunk: {}/{:?}", chunk_char_start, s.len());

                let mut x = x_base;
                if let Cursor::Selection { range, .. } = &buffer.cursor() {
                    if in_selection && *range.back() == Point::new(y, x) {
                        in_selection = false;
                        queue!(stdout, SetAttribute(Attribute::NoReverse)).ok();
                    } else if !in_selection && *range.front() == Point::new(y, x) {
                        in_selection = true;
                        queue!(stdout, SetAttribute(Attribute::Reverse)).ok();
                    }
                }

                chunk_i = 0;
                for c in s.chars().skip(chunk_char_start) {
                    let (tab, char_width) = match c {
                        '\t' => (
                            true,
                            buffer.config().tab_width - x % buffer.config().tab_width,
                        ),
                        _ => (false, c.display_width()),
                    };

                    if char_width > remaining_width {
                        break 'outer;
                    }

                    if y == main_pos.y && x == main_pos.x {
                        cursor_pos = Some(Point::new(display_y, display_x));
                    }

                    if let Some(pos) = cursor_hover {
                        if y == pos.y && x == pos.x {
                            queue!(stdout, SetBackgroundColor(Color::DarkGrey)).ok();
                        }
                    }

                    if tab {
                        queue!(stdout, Print(whitespaces(char_width))).ok();
                        for i in 0..char_width {
                            self.pixel_map.set_pixel(
                                &Point::new(1 + display_y, display_x_text_start + display_x + i),
                                &Point::new(y, x),
                            );
                        }
                    } else {
                        queue!(stdout, Print(c)).ok();
                        self.pixel_map.set_pixel(
                            &Point::new(1 + display_y, display_x_text_start + display_x),
                            &Point::new(y, x),
                        );
                    }

                    // Clear the cursor hover.
                    queue!(stdout, SetBackgroundColor(Color::Reset)).ok();

                    remaining_width -= char_width;
                    chunk_i += 1;
                    display_x += char_width;
                    x += 1;
                    x_base += 1;
                    end_x = max(x, end_x);

                    if let Cursor::Selection { range, .. } = &buffer.cursor() {
                        if in_selection && *range.back() == Point::new(y, x) {
                            in_selection = false;
                            queue!(stdout, SetAttribute(Attribute::NoReverse)).ok();
                        } else if !in_selection && *range.front() == Point::new(y, x) {
                            in_selection = true;
                            queue!(stdout, SetAttribute(Attribute::Reverse)).ok();
                        }

                        // Print ' ' at the end of line if the newline character is selected.
                        if in_selection && range.back().y > y && x == buffer.line_len(y) {
                            queue!(stdout, Print(' ')).ok();
                        }
                    }
                }

                // Printed all characters in the chunk. Visit the next one.
                chunks.next();
                chunk_char_start = 0;
            }

            // Handle the cursor at the end of line.
            if y == main_pos.y && main_pos.x == buffer.line_len(main_pos.y) {
                cursor_pos = Some(Point::new(display_y, display_x));
            }

            if remaining_width > 0 {
                if let Some(pos) = cursor_hover {
                    if y == pos.y && end_x <= pos.x {
                        queue!(
                            stdout,
                            SetBackgroundColor(Color::DarkGrey),
                            Print(' '),
                            SetBackgroundColor(Color::Reset)
                        )
                        .ok();
                    }
                }
            }

            self.pixel_map
                .set_pixel(&Point::new(1 + display_y, 0), &Point::new(y, end_x));

            queue!(stdout, Clear(ClearType::UntilNewLine)).ok();

            match chunks.peek() {
                Some(_) => {
                    // There're remaining unprinted chunks in the line, i.e.,
                    // we need line wrapping.
                    wrapped = Some((chunks, chunk_char_start + chunk_i));
                }
                None => {
                    // Printed all chunks in the line.
                    wrapped = None;
                    y += 1;
                }
            }
        }

        // The status line.
        //           notification
        //          VVVVVVVVVVVVVVV
        //     (25) saved {} lines                  main.c [+]
        //     ^^^^^- column #             name ----^^^^^^ ^^^--- dirty idicator
        //
        let colno_width = num_of_digits(main_pos.x) + 3;
        let indicator = if buffer.is_dirty() { " [+]" } else { "" };
        let indicator_width = indicator.display_width();
        let notification_width = min(
            notification.map(|n| n.message.display_width()).unwrap_or(0) + 1,
            self.cols - (colno_width + 1 /* pad */ + 10 /* name width */ + indicator_width),
        );
        let buffer_name = buffer.name();
        let name_width = min(
            buffer_name.display_width(),
            self.cols - (colno_width + notification_width + indicator_width),
        );
        queue!(
            stdout,
            MoveTo(0, 0),
            SetForegroundColor(Color::DarkGrey),
            Print('('),
            Print(main_pos.x + 1),
            Print(") "),
            SetAttribute(Attribute::Bold),
            match notification.map(|n| n.level) {
                Some(NotificationLevel::Info) => SetForegroundColor(Color::Green),
                Some(NotificationLevel::Error) => SetForegroundColor(Color::Red),
                None => SetForegroundColor(Color::Reset),
            },
            Print(with_ellipsis(
                notification.map(|n| n.message.as_str()).unwrap_or(""),
                notification_width
            )),
            Clear(ClearType::UntilNewLine),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::Cyan),
            MoveTo((self.cols - (name_width + indicator_width)) as u16, 0),
            Print(with_ellipsis(buffer_name, name_width)),
            SetForegroundColor(Color::Yellow),
            Print(indicator),
            SetAttribute(Attribute::Reset),
        )
        .ok();

        // Move and show the cursor.
        if let Some(Point { y, x }) = cursor_pos {
            queue!(
                stdout,
                MoveTo((display_x_text_start + x) as u16, 1 + y as u16),
                cursor::Show
            )
            .ok();
        }

        stdout.flush().ok();
    }

    pub fn draw_buffer(
        &mut self,
        buffer: &mut Buffer,
        notification: Option<&Notification>,
        cursor_hover: Option<&Point>,
    ) {
        let mut stdout = stdout();
        if self.cols < 20 || self.rows < 5 {
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

        queue!(stdout, cursor::Hide).ok();

        self.pixel_map.clear(self.rows, self.cols);

        let lineno_max_width = num_of_digits(buffer.num_lines()) + 1;
        let display_y_text_start = 1;
        let display_x_text_start = lineno_max_width;

        self.display_x_text_start = display_x_text_start;
        self.text_cols = self.cols - display_x_text_start;

        // Adjust top left.
        buffer.adjust_top_left(
            self.rows - display_y_text_start,
            self.cols - display_x_text_start,
        );
        let top_left = buffer.top_left();
        self.current_top_left = top_left.clone();

        let main_pos = buffer.main_cursor_pos();

        trace!("main_pos={}, top_left={:?}", main_pos, top_left);

        use ropey::iter::Chunks;
        use std::iter::Peekable;

        let mut pos = Point::new(top_left.y, top_left.x);
        let mut wrapped: Option<(Peekable<Chunks>, usize)> = if top_left.x == 0 {
            None
        } else {
            let mut chunks = buffer.line(top_left.y).chunks().peekable();
            let (chunk_i, char_i) = buffer.line_substr_chunk(top_left.y, top_left.x).unwrap();
            for chunk in 0..chunk_i {
                chunks.next();
            }
            Some((chunks, char_i))
        };
        let mut cursor_pixel = None;
        for display_y in display_y_text_start..self.rows {
            // Move the cursor at the beginning of the next display row.
            queue!(
                stdout,
                MoveTo(0, display_y as u16),
                SetAttribute(Attribute::Reset),
            )
            .ok();

            self.draw_line_number(
                &mut stdout,
                pos.y + 1,
                lineno_max_width,
                pos.y == main_pos.y,
                wrapped.is_some(),
            );

            // Get the string chunks of the current (or next) buffer line.
            let (mut chunks, mut chunk_start_idx) = match wrapped {
                Some(inner) => inner,
                None if pos.y < buffer.num_lines() => (buffer.line(pos.y).chunks().peekable(), 0),
                None => {
                    // Out of bounds of the buffer.
                    queue!(
                        stdout,
                        SetAttribute(Attribute::Reset),
                        Clear(ClearType::CurrentLine),
                    )
                    .ok();
                    continue;
                }
            };

            // Render chunks until the end of the display row.
            let mut pixel = Pixel::new(display_y, display_x_text_start);
            let mut chunk_printed_idx = 0;
            'outer: loop {
                let s = match chunks.peek() {
                    Some(s) => s,
                    None => break,
                };

                trace!(
                    "d_y={}, chunk={:?}, start={}, len={}",
                    pixel.y,
                    s.as_ptr(),
                    chunk_start_idx,
                    s.len()
                );

                for c in s.chars().skip(chunk_start_idx) {
                    let (tab, width) = match c {
                        '\t' => (
                            true,
                            buffer.config().tab_width - pos.x % buffer.config().tab_width,
                        ),
                        _ => (false, c.display_width()),
                    };

                    if pixel.x + width > self.cols {
                        break 'outer;
                    }

                    if pos == *main_pos {
                        cursor_pixel = Some(pixel.clone());
                    }

                    if tab {
                        queue!(stdout, Print(whitespaces(width))).ok();
                    } else {
                        queue!(stdout, Print(c)).ok();
                    }

                    pos.x += 1;
                    chunk_printed_idx += 1;
                    pixel.x += width;
                }

                // Printed all characters in the chunk. Visit the next one.
                chunks.next();
                chunk_start_idx = 0;
                chunk_printed_idx = 0;
            }

            // Clear the previously printed contents.
            queue!(stdout, Clear(ClearType::UntilNewLine)).ok();

            match chunks.peek() {
                Some(_) => {
                    // There're remaining unprinted chunks in the line, i.e.,
                    // we need line wrapping.
                    wrapped = Some((chunks, chunk_start_idx + chunk_printed_idx));
                }
                None => {
                    // Printed all chunks in the line.
                    wrapped = None;
                    pos.y += 1;
                    pos.x = 0;
                }
            }
        }

        self.draw_status_line(&mut stdout, buffer, &notification, &main_pos);

        // Move and show the cursor.
        if let Some(Pixel { y, x }) = cursor_pixel {
            queue!(stdout, MoveTo(x as u16, y as u16), cursor::Show).ok();
        }

        stdout.flush().ok();
    }

    pub fn draw_line_number(
        &self,
        stdout: &mut std::io::Stdout,
        lineno: usize,
        lineno_max_width: usize,
        active: bool,
        wrapped: bool,
    ) {
        if wrapped {
            queue!(
                stdout,
                SetForegroundColor(Color::DarkGrey),
                Print(whitespaces(lineno_max_width - 2)),
                Print("~"),
                SetAttribute(Attribute::Reset),
                Print(' '),
            )
            .ok();
        } else {
            if active {
                queue!(stdout, SetAttribute(Attribute::Bold)).ok();
            } else {
                queue!(stdout, SetForegroundColor(Color::DarkGrey)).ok();
            }

            queue!(
                stdout,
                Print(whitespaces(lineno_max_width - num_of_digits(lineno) - 1)),
                Print(lineno),
                Print(' '),
                SetAttribute(Attribute::Reset),
            )
            .ok();
        }
    }

    fn draw_status_line(
        &self,
        stdout: &mut std::io::Stdout,
        buffer: &Buffer,
        notification: &Option<&Notification>,
        main_pos: &Point,
    ) {
        //           notification
        //          VVVVVVVVVVVVVVV
        //     (25) saved {} lines                  main.c [+]
        //     ^^^^^- column #             name ----^^^^^^ ^^^--- dirty idicator
        let colno_width = num_of_digits(main_pos.x) + 3;
        let indicator = if buffer.is_dirty() { " [+]" } else { "" };
        let indicator_width = indicator.display_width();
        let notification_width = min(
            notification.map(|n| n.message.display_width()).unwrap_or(0) + 1,
            self.cols - (colno_width + 1 /* pad */ + 10 /* name width */ + indicator_width),
        );
        let buffer_name = buffer.name();
        let name_width = min(
            buffer_name.display_width(),
            self.cols - (colno_width + notification_width + indicator_width),
        );
        queue!(
            stdout,
            MoveTo(0, 0),
            SetForegroundColor(Color::DarkGrey),
            Print('('),
            Print(main_pos.x + 1),
            Print(") "),
            SetAttribute(Attribute::Bold),
            match notification.map(|n| n.level) {
                Some(NotificationLevel::Info) => SetForegroundColor(Color::Green),
                Some(NotificationLevel::Error) => SetForegroundColor(Color::Red),
                None => SetForegroundColor(Color::Reset),
            },
            Print(with_ellipsis(
                notification.map(|n| n.message.as_str()).unwrap_or(""),
                notification_width
            )),
            Clear(ClearType::UntilNewLine),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::Cyan),
            MoveTo((self.cols - (name_width + indicator_width)) as u16, 0),
            Print(with_ellipsis(buffer_name, name_width)),
            SetForegroundColor(Color::Yellow),
            Print(indicator),
            SetAttribute(Attribute::Reset),
        )
        .ok();
    }

    pub fn draw_finder(&mut self, finder: &Finder, input: &mut LineEdit) {
        let text_width = self.cols - 8;
        input.adjust_top_left(text_width);

        let text = &input.text();
        let text_index = text
            .char_indices()
            .nth(input.top_left())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let mut stdout = stdout();
        queue!(
            stdout,
            cursor::Hide,
            MoveTo(0, 0),
            SetAttribute(Attribute::Reverse),
            Print(" FIND "),
            SetAttribute(Attribute::NoReverse),
            Print(' '),
            Print(truncate(&text[text_index..], text_width)),
            Clear(ClearType::UntilNewLine),
            MoveTo(0, 1),
            Clear(ClearType::UntilNewLine),
        )
        .ok();

        let mut y = 2;
        let mut y_remaining = min(self.rows - 1, 15);
        for (i, item) in finder.items().iter().enumerate() {
            queue!(stdout, MoveTo(0, y)).ok();
            if i == finder.selected_item_index() {
                queue!(
                    stdout,
                    SetAttribute(Attribute::Bold),
                    SetAttribute(Attribute::Underlined)
                )
                .ok();
            }

            let suffix = match &item.data {
                FinderItem::File { .. } => "file",
                FinderItem::Buffer { .. } => "buffer",
            };

            let suffix_width = suffix.display_width();
            let item_width_max = self.cols - (1 + suffix_width);

            let item_width = match &item.data {
                FinderItem::File { path, pos: None } => {
                    let s = truncate(path.to_str().unwrap(), item_width_max);
                    queue!(stdout, Print(s)).ok();
                    s.display_width()
                }
                FinderItem::File {
                    path: _,
                    pos: Some(_pos),
                } => {
                    unimplemented!();
                }
                FinderItem::Buffer { path } => {
                    let s = truncate(path.to_str().unwrap(), item_width_max);
                    queue!(stdout, Print(s)).ok();
                    s.display_width()
                }
            };

            queue!(
                stdout,
                Print(whitespaces(item_width_max - item_width)),
                Print(suffix),
                SetAttribute(Attribute::Reset),
            )
            .ok();

            y += 1;
            y_remaining -= 1;
            if y_remaining == 0 {
                break;
            }
        }

        for _ in 0..y_remaining {
            queue!(stdout, MoveTo(0, y), Clear(ClearType::UntilNewLine)).ok();
            y += 1;
        }

        queue!(
            stdout,
            MoveTo((7 + input.cursor() - input.top_left()) as u16, 0),
            cursor::Show,
        )
        .ok();

        stdout.flush().ok();
    }

    fn in_text_area(&self, y: u16, x: u16) -> Option<Point> {
        let in_text_area = (y as usize) >= 1 && self.display_x_text_start <= (x as usize);
        if !in_text_area {
            return None;
        }

        self.pixel_map
            .display_pos_to_text_pos(&Point::new(y as usize, x as usize))
            .or_else(|| {
                self.pixel_map
                    .display_pos_to_text_pos(&Point::new(y as usize, 0))
            })
            .cloned()
    }

    pub fn convert_raw_mouse_event(&mut self, ev: RawMouseEvent) -> Option<MouseEvent> {
        const LEFT: MouseButton = MouseButton::Left;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        match (ev, self.last_clicked) {
            (
                RawMouseEvent {
                    kind: MouseEventKind::Down(LEFT),
                    column,
                    row,
                    ..
                },
                _,
            ) if (column as usize) < self.display_x_text_start => Some(MouseEvent::ClickLineNo {
                y: self.current_top_left.y + row as usize - 1,
            }),
            (
                RawMouseEvent {
                    kind: MouseEventKind::Down(LEFT),
                    column,
                    row,
                    modifiers,
                },
                Some(last_clicked),
            ) if last_clicked.elapsed() < Duration::from_millis(200) => {
                self.last_clicked = None;
                self.in_text_area(row, column)
                    .map(|pos| MouseEvent::DoubleClickText {
                        pos,
                        alt: modifiers == ALT,
                    })
            }
            (
                RawMouseEvent {
                    kind: MouseEventKind::Down(LEFT),
                    column,
                    row,
                    modifiers,
                },
                _,
            ) => {
                self.last_clicked = Some(Instant::now());
                self.in_text_area(row, column)
                    .map(|pos| MouseEvent::ClickText {
                        pos,
                        alt: modifiers == ALT,
                    })
            }
            (
                RawMouseEvent {
                    kind: MouseEventKind::Drag(LEFT),
                    row,
                    column,
                    ..
                },
                _,
            ) if (column as usize) < self.display_x_text_start => {
                Some(MouseEvent::DragLineNo { y: row as usize })
            }
            (
                RawMouseEvent {
                    kind: MouseEventKind::Drag(LEFT),
                    row,
                    column,
                    ..
                },
                _,
            ) => self
                .in_text_area(row, column)
                .map(|pos| MouseEvent::DragText { pos }),
            (
                RawMouseEvent {
                    kind: MouseEventKind::ScrollDown,
                    ..
                },
                _,
            ) => Some(MouseEvent::ScrollDown),
            (
                RawMouseEvent {
                    kind: MouseEventKind::ScrollUp,
                    ..
                },
                _,
            ) => Some(MouseEvent::ScrollUp),
            (
                RawMouseEvent {
                    kind: MouseEventKind::Moved,
                    row,
                    column,
                    ..
                },
                _,
            ) => self
                .in_text_area(row, column)
                .map(|pos| MouseEvent::HoverText { pos }),
            _ => None,
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen).ok();
        execute!(stdout(), DisableMouseCapture).ok();
        disable_raw_mode().ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_ellipsis() {
        assert_eq!(with_ellipsis("123456789", 0), "");
        assert_eq!(with_ellipsis("123456789", 1), ".");
        assert_eq!(with_ellipsis("123456789", 3), "...");
        assert_eq!(with_ellipsis("123456789", 9), "123456789");
        assert_eq!(with_ellipsis("123456789", 5), "1...9");
        assert_eq!(with_ellipsis("123456789", 6), "1...89");
        assert_eq!(with_ellipsis("123456789abcdefg", 10), "123...defg");
    }
}
