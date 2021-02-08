use crate::buffer::{Buffer, TopLeft};
use crate::editor::{Event, Notification, NotificationLevel};
use crate::editorconfig::EditorConfig;
use crate::finder::{Finder, FinderItem};
use crate::line_edit::LineEdit;
use crate::ned;
use crate::rope::{Cursor, Point, Range, Rope};
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

struct PixelMap {
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

    pub fn set_pixel(&mut self, pixel: &Pixel, pos: &Point) {
        let index = self.pixel_to_index(pixel);
        self.map[index] = Some(pos.clone());
    }

    pub fn pixel_to_pos(&self, pixel: &Pixel) -> Option<&Point> {
        trace!(
            "pixel: {:?} -> {:?}",
            pixel,
            self.map.get(self.pixel_to_index(pixel))
        );
        self.map
            .get(self.pixel_to_index(pixel))
            .map(|o| o.as_ref())
            .unwrap_or(None)
    }

    fn pixel_to_index(&self, pixel: &Pixel) -> usize {
        pixel.y * self.width + pixel.x
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

        let selections = match buffer.cursor() {
            Cursor::Selection { range, .. } => vec![range.clone()],
            Cursor::Normal { .. } => vec![],
        };

        let lineno_max_width = num_of_digits(buffer.num_lines()) + 1;
        let display_y_text_start = 1;
        let display_x_text_start = lineno_max_width;

        self.display_x_text_start = display_x_text_start;
        self.text_cols = self.cols - display_x_text_start;

        // Adjust top left.
        buffer.relocate_top_left(
            self.rows - display_y_text_start,
            self.cols - display_x_text_start,
        );
        self.current_top_left = buffer.top_left().clone();

        queue!(stdout, cursor::Hide).ok();
        self.pixel_map.clear(self.rows, self.cols);
        let cursor_pixel = self.draw_text(
            &mut stdout,
            self.rows,
            display_y_text_start,
            lineno_max_width,
            buffer.rope(),
            buffer.top_left(),
            buffer.main_cursor_pos(),
            cursor_hover,
            buffer.config().tab_width,
            &selections,
        );
        self.draw_status_line(
            &mut stdout,
            buffer,
            &notification,
            &buffer.main_cursor_pos(),
        );
        queue!(
            stdout,
            MoveTo(cursor_pixel.x as u16, cursor_pixel.y as u16),
            cursor::Show
        )
        .ok();

        stdout.flush().ok();
    }

    fn draw_text(
        &mut self,
        stdout: &mut std::io::Stdout,
        height: usize,
        display_y_start: usize,
        lineno_max_width: usize,
        rope: &Rope,
        top_left: &TopLeft,
        cursor_pos: &Point,
        cursor_hover: Option<&Point>,
        tab_width: usize,
        selections: &[Range],
    ) -> Pixel {
        let mut pos = Point::new(top_left.y, top_left.x);
        let mut wrapped = None;
        let mut in_selection = false;
        let mut cursor_pixel = Pixel::new(0, 0);
        for display_y in display_y_start..height {
            // Move the cursor at the beginning of the next display row.
            queue!(
                stdout,
                MoveTo(0, display_y as u16),
                SetAttribute(Attribute::Reset),
            )
            .ok();

            // Get the string chunks of the current (or next) buffer line.
            let is_wrapped = wrapped.is_some();
            let (mut chunks, mut chunk_start_idx) = match wrapped {
                Some(inner) => inner,
                None if pos.y < rope.num_lines() => (rope.line(pos.y).chunks().peekable(), 0),
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

            // The pos.y is in the bufer. Print the line number.
            self.draw_line_number(
                stdout,
                pos.y + 1,
                lineno_max_width,
                pos.y == cursor_pos.y,
                is_wrapped,
            );

            let mut pixel = Pixel::new(display_y, lineno_max_width);

            // Handle the cursor at the beginning of the line.
            if pos == *cursor_pos {
                cursor_pixel = pixel.clone();
            }

            // Render chunks until the end of the display row.
            let mut chunk_printed_idx = 0;
            'outer: loop {
                let s = match chunks.peek() {
                    Some(s) => s,
                    None => break,
                };

                for c in s.chars().skip(chunk_start_idx) {
                    let (tab, width) = match c {
                        '\t' => (true, tab_width - pos.x % tab_width),
                        _ => (false, c.display_width()),
                    };

                    if pos == *cursor_pos && is_wrapped {
                        cursor_pixel = pixel.clone();
                    }

                    if pixel.x + width > self.cols {
                        break 'outer;
                    }

                    for range in selections {
                        if in_selection && pos >= *range.back() {
                            in_selection = false;
                            queue!(stdout, SetAttribute(Attribute::NoReverse)).ok();
                        } else if (in_selection && chunk_printed_idx == 0)
                            || (!in_selection && pos == *range.front())
                        {
                            in_selection = true;
                            queue!(stdout, SetAttribute(Attribute::Reverse)).ok();
                        }
                    }

                    let on_hover = match cursor_hover {
                        Some(hover_pos) if pos == *hover_pos => {
                            queue!(stdout, SetBackgroundColor(Color::DarkGrey)).ok();
                            true
                        }
                        _ => false,
                    };

                    if tab {
                        queue!(stdout, Print(whitespaces(width))).ok();
                    } else {
                        queue!(stdout, Print(c)).ok();
                    }

                    if on_hover {
                        queue!(stdout, SetBackgroundColor(Color::Reset)).ok();
                    }

                    for i in 0..width {
                        self.pixel_map
                            .set_pixel(&Pixel::new(pixel.y, pixel.x + i), &pos);
                    }

                    pos.x += 1;
                    chunk_printed_idx += 1;
                    pixel.x += width;

                    if pos == *cursor_pos {
                        cursor_pixel = pixel.clone();
                    }
                }

                // Printed all characters in the chunk. Visit the next one.
                chunks.next();
                chunk_start_idx = 0;
                chunk_printed_idx = 0;
            }

            // Clear the previously printed contents.
            queue!(stdout, Clear(ClearType::UntilNewLine)).ok();

            self.pixel_map.set_pixel(&Pixel::new(pixel.y, 0), &pos);

            match chunks.peek() {
                Some(_) => {
                    // There're remaining unprinted chunks in the line, i.e.,
                    // we need line wrapping.
                    wrapped = Some((chunks, chunk_start_idx + chunk_printed_idx));
                }
                None => {
                    // Printed all chunks in the line.

                    // Handle the cursor at the end of the line.
                    if pos == *cursor_pos {
                        cursor_pixel = pixel.clone();
                    }

                    // Print ' ' at the end of line if the newline character is selected.
                    for range in selections {
                        if in_selection && range.back().y > pos.y {
                            queue!(stdout, Print(' ')).ok();
                        }
                    }

                    // Handle cursor hover at the end of tha line.
                    if matches!(cursor_hover, Some(cursor_pos) if pos.y == cursor_pos.y && pos.x <= cursor_pos.x)
                    {
                        queue!(
                            stdout,
                            SetBackgroundColor(Color::DarkGrey),
                            Print(' '),
                            SetBackgroundColor(Color::Reset),
                        )
                        .ok();
                    }

                    wrapped = None;
                    pos.y += 1;
                    pos.x = 0;
                }
            }
        }

        cursor_pixel
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

    fn draw_prompt(
        &self,
        stdout: &mut std::io::Stdout,
        title: &str,
        input: &mut LineEdit,
    ) -> (u16, u16) {
        let text_width = self.cols - 8;
        input.relocate_top_left(text_width);

        let text = &input.text();
        let text_index = text
            .char_indices()
            .nth(input.top_left())
            .map(|(i, _)| i)
            .unwrap_or(0);

        queue!(
            stdout,
            MoveTo(0, 1),
            SetAttribute(Attribute::Reverse),
            Print(format!(" {} ", title)),
            SetAttribute(Attribute::NoReverse),
            Print(' '),
            Print(truncate(&text[text_index..], text_width)),
            Clear(ClearType::UntilNewLine),
        )
        .ok();

        (1, (7 + input.cursor() - input.top_left()) as u16)
    }

    fn draw_status_line(
        &self,
        stdout: &mut std::io::Stdout,
        buffer: &Buffer,
        notification: &Option<&Notification>,
        cursor_pos: &Point,
    ) {
        //           notification
        //          VVVVVVVVVVVVVVV
        //     (25) saved {} lines                  main.c [+]
        //     ^^^^^- column #             name ----^^^^^^ ^^^--- dirty idicator
        let colno_width = num_of_digits(cursor_pos.x) + 3;
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
            Print(cursor_pos.x + 1),
            Print(") "),
            SetAttribute(Attribute::Bold),
            match notification.map(|n| n.level) {
                Some(NotificationLevel::Info) => SetForegroundColor(Color::Green),
                Some(NotificationLevel::Warn) => SetForegroundColor(Color::Yellow),
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
        let mut stdout = stdout();
        queue!(stdout, cursor::Hide).ok();
        let (cursor_y, cursor_x) = self.draw_prompt(&mut stdout, "FIND", input);

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

        queue!(stdout, MoveTo(cursor_x, cursor_y), cursor::Show).ok();
        stdout.flush().ok();
    }

    pub fn draw_ned(
        &mut self,
        buffer: &Buffer,
        preview_rope: &Rope,
        changes: Option<&ned::Changes>,
        input: &mut LineEdit,
        notification: Option<&Notification>,
    ) {
        let text_width = self.cols - 8;
        input.relocate_top_left(text_width);

        let text = &input.text();
        let text_index = text
            .char_indices()
            .nth(input.top_left())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let y = changes
            .and_then(|c| c.last_matches.get(0))
            .map(|m| m.range.front().y)
            .unwrap_or(0);

        let selections = changes
            .map(|changes| {
                changes
                    .last_matches
                    .iter()
                    .map(|m| m.range.clone())
                    .collect()
            })
            .unwrap_or_else(|| Vec::new());

        let mut stdout = stdout();
        queue!(stdout, cursor::Hide).ok();
        self.draw_text(
            &mut stdout,
            self.rows,
            2,
            num_of_digits(y + self.rows - 2) + 1,
            preview_rope,
            &TopLeft::new(y, 0),
            &Point::new(y, 0),
            None,
            buffer.config().tab_width,
            &selections,
        );
        self.draw_status_line(&mut stdout, buffer, &notification, buffer.main_cursor_pos());
        let (cursor_y, cursor_x) = self.draw_prompt(&mut stdout, "EDIT", input);

        queue!(stdout, MoveTo(cursor_x, cursor_y), cursor::Show).ok();
        stdout.flush().ok();
    }

    fn in_text_area(&self, y: u16, x: u16) -> Option<Point> {
        let in_text_area = (y as usize) >= 1 && self.display_x_text_start <= (x as usize);
        if !in_text_area {
            return None;
        }

        self.pixel_map
            .pixel_to_pos(&Pixel::new(y as usize, x as usize))
            .or_else(|| self.pixel_map.pixel_to_pos(&Pixel::new(y as usize, 0)))
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
