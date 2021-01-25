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
use std::cmp::min;
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

pub struct Terminal {
    rows: usize,
    cols: usize,
    text_cols: usize,
    current_top_left: TopLeft,
    current_num_lines: usize,
    last_clicked: Option<Instant>,
    text_start_x: usize,
    text_end_x: usize,
    text_height: usize,
    cursor_text_map: HashMap<Point, Point>,
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
            current_num_lines: 0,
            last_clicked: None,
            text_start_x: 0,
            text_end_x: 0,
            text_height: 0,
            cursor_text_map: HashMap::new(),
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

        self.cursor_text_map.clear();

        let lineno_width = num_of_digits(buffer.num_lines()) + 1;
        let text_offset = lineno_width;
        let text_height = self.rows - 1;
        let text_width = self.cols - text_offset;
        self.text_cols = text_width;

        // Adjust top left.
        buffer.adjust_top_left(text_height, text_width);
        let top_left = buffer.top_left();
        self.current_top_left = top_left.clone();
        self.current_num_lines = buffer.num_lines();
        self.text_start_x = text_offset;
        self.text_end_x = text_offset + text_width;
        self.text_height = text_height;

        let main_pos = buffer.main_cursor_pos();

        // Draw the text area.
        let mut y = top_left.y;
        let mut wrapped = None;
        let mut cursor_pos = None;
        let mut in_selection = false;
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
                cursor_pos = Some((display_y, 0));
            }

            // Get the string chunks of the current (or next) line.
            let is_wrapped = wrapped.is_some();
            let (mut chunks, chunk_char_start) = match wrapped {
                Some(inner) => inner,
                None if y < buffer.num_lines() => (buffer.line(y).chunks().peekable(), 0),
                None => {
                    // Out of bounds.
                    queue!(
                        stdout,
                        SetAttribute(Attribute::Reset),
                        Clear(ClearType::UntilNewLine),
                    )
                    .ok();
                    continue;
                }
            };

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
            'outer: while remaining_width > 0 {
                let s = match chunks.peek() {
                    Some(s) => s,
                    None => break,
                };

                let mut x = chunk_char_start;
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
                        cursor_pos = Some((display_y, display_x));
                    }

                    if let Some(pos) = cursor_hover {
                        if y == pos.y && x == pos.x {
                            queue!(stdout, SetBackgroundColor(Color::DarkGrey)).ok();
                        }
                    }

                    if tab {
                        queue!(stdout, Print(whitespaces(char_width))).ok();
                        for i in 0..char_width {
                            self.cursor_text_map.insert(
                                Point::new(1 + display_y, text_offset + display_x + i),
                                Point::new(y, x),
                            );
                        }
                    } else {
                        queue!(stdout, Print(c)).ok();
                        self.cursor_text_map.insert(
                            Point::new(1 + display_y, text_offset + display_x),
                            Point::new(y, x),
                        );
                    }

                    // Clear the cursor hover.
                    queue!(stdout, SetBackgroundColor(Color::Reset)).ok();

                    remaining_width -= char_width;
                    chunk_i += 1;
                    display_x += char_width;
                    x += 1;

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
            }

            // Handle the cursor at the end of line.
            if y == main_pos.y && main_pos.x == buffer.line_len(main_pos.y) {
                cursor_pos = Some((display_y, display_x));
            }

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
        if let Some((y, x)) = cursor_pos {
            queue!(
                stdout,
                MoveTo((text_offset + x) as u16, 1 + y as u16),
                cursor::Show
            )
            .ok();
        }

        stdout.flush().ok();
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
            queue!(stdout, MoveTo(0, y), Clear(ClearType::UntilNewLine),).ok();
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
        let in_text_area = (y as usize) >= 1
            && self.text_start_x <= (x as usize)
            && (x as usize) <= self.text_end_x;

        if !in_text_area {
            return None;
        }

        match self
            .cursor_text_map
            .get(&Point::new(y as usize, x as usize))
        {
            Some(buffer_pos) => Some(buffer_pos.clone()),
            None => {
                // FIXME:
                let pos_y = self.current_top_left.y + y as usize - 1;
                let pos_x = x as usize - self.text_start_x;
                Some(Point::new(pos_y, pos_x))
            }
        }
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
            ) if (column as usize) < self.text_start_x => Some(MouseEvent::ClickLineNo {
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
            ) if last_clicked.elapsed() < Duration::from_millis(400) => {
                self.last_clicked = Some(Instant::now());
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
            ) if (column as usize) < self.text_start_x => {
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
mod test {
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
