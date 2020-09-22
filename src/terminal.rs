use crate::editor::{EventQueue, Event, Notification, Popup};
use crate::rope::Cursor;
use crate::view::View;
use crate::highlight::{Highlighter};
use crate::theme::{THEME, ThemeItem};
use crate::command_box::{CommandBox, ResponseBody, PreviewItem};
use std::cmp::{min, max};
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
use crate::buffer::Buffer;
use crate::view::TopLeft;
use crate::status_map::{StatusMap, LineStatus};

pub fn truncate(s: &str, width: usize) -> &str {
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
        notifications: &[Notification],
        popup: &Option<Popup>,
        command_box: Option<(&CommandBox, &Buffer)>,
        status_map: &StatusMap,
    ) {
        use unicode_width::{UnicodeWidthChar};
        use crossterm::cursor::{self, MoveTo};
        use crossterm::terminal::{Clear, ClearType};
        use crossterm::style::{
            Print, SetForegroundColor, SetBackgroundColor,
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
        let mut buffer = view.buffer().borrow_mut();
        let top_left = view.top_left();

        // TODO: cache
        // Highlight the given text.
        // let modified_line = buffer.modified_line().unwrap_or(0);
        // if top_left.y <= modified_line && modified_line <= top_left.y + text_height {
            let modified_line = 0;
            let range = modified_line..=modified_line + text_height;
            buffer.highlight(range);
        // }

        // Hide the cursor to prevent flickering.
        queue!(stdout,
            cursor::Hide,
        ).unwrap();

        // Draw buffer contents.
        use std::collections::HashMap;
        let mut scroll_bar_y = 0;
        let scroll_bar_diff = max(
            1,
            (buffer.num_lines() as f64 / text_height as f64).ceil() as usize
        );
        for i in 0..text_height {
            queue!(stdout,
                MoveTo(0, i as u16),
                SetAttribute(Attribute::Reset),
            ).unwrap();

            // Line number.
            let y = top_left.y + i;
            let lineno = y + 1; // 1-origin
            let out_of_bounds = lineno > buffer.num_lines();
            if out_of_bounds {
                THEME.apply(&mut stdout, ThemeItem::LineNo).ok();
                queue!(stdout,
                    Print(whitespaces(lineno_width)),
                    SetAttribute(Attribute::Reset),
                ).unwrap();
            } else {
                THEME.apply(&mut stdout, ThemeItem::LineNoPadding).ok();
                queue!(stdout,
                    Print(whitespaces(lineno_width - num_of_digits(lineno) - 1)),
                    Print(lineno),
                    Print(" "),
                    SetAttribute(Attribute::Reset),
                ).unwrap();
            }

            // Line map.
            if let Some(LineStatus { status, .. }) = status_map.get(y) {
                THEME.apply(&mut stdout, ThemeItem::LineStatus(*status)).ok();
                queue!(stdout,
                    Print(' '),
                    SetAttribute(Attribute::Reset),
                ).unwrap();
            } else {
                THEME.apply(&mut stdout, ThemeItem::LineStatusPadding).ok();
                queue!(stdout,
                    Print(' '),
                    SetAttribute(Attribute::Reset),
                ).unwrap();
            }

            // Margin.
            queue!(stdout, Print(' ')).unwrap();

            // Text.
            if !out_of_bounds {
                let line = buffer.line(y);
                if line.len_chars() > top_left.x {
                    self.draw_text_line(
                        &mut stdout,
                        &buffer,
                        &line,
                        &top_left,
                        y,
                        text_width,
                    );
                }
            }

            queue!(
                stdout,
                Clear(ClearType::UntilNewLine),
                MoveTo(self.cols as u16 - 1, i as u16),
            ).unwrap();

            // Scroll bar.
            trace!("sc={}..{}", scroll_bar_y, scroll_bar_y + scroll_bar_diff);
            if let Some(LineStatus { status, .. })
                = status_map.get_by_range(scroll_bar_y, scroll_bar_diff) {
                THEME.apply(&mut stdout, ThemeItem::LineStatus(*status)).ok();
                queue!(stdout,
                    Print(' '),
                    SetAttribute(Attribute::Reset),
                ).unwrap();
            } else if top_left.y <= scroll_bar_y + scroll_bar_diff
                && scroll_bar_y <= top_left.y + text_height {
                THEME.apply(&mut stdout, ThemeItem::ScrollBarVisible).ok();
                queue!(stdout,
                    Print(' '),
                    SetAttribute(Attribute::Reset),
                ).unwrap();
            }

            scroll_bar_y += scroll_bar_diff;
        }

        // Draw the info bar.
        THEME.apply(&mut stdout, ThemeItem::InfoBarColor).ok();
        queue!(stdout,
            MoveTo(0, status_bar_y as u16),
            Print(" "),
            SetAttribute(Attribute::Bold),
            SetAttribute(Attribute::Underlined),
            Print(buffer.name()),
            SetAttribute(Attribute::NoUnderline),
            Print(" "),
            SetAttribute(Attribute::Reset),
        ).unwrap();

        if buffer.is_dirty() {
            THEME.apply(&mut stdout, ThemeItem::DirtyBufferMark).ok();
            queue!(stdout,
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
                    popup.items().iter().map(String::len).max().unwrap_or(0);
                let popup_width = min(longest + 1, text_width - 3);
                let x = if cursor_x + popup_width < text_width {
                    cursor_x
                } else {
                    text_width - popup_width
                };

                let (y, popup_height) = if cursor_y + 1 + popup.len() < text_height {
                    (cursor_y + 1, popup.len())
                } else {
                    (cursor_y + 1, text_height - cursor_y - 1)
                };

                for i in 0..popup_height {
                    let item = &popup.items()[i];
                    if i == popup.selected() {
                        THEME.apply(&mut stdout, ThemeItem::PopupItemHover).ok();
                    } else {
                        THEME.apply(&mut stdout, ThemeItem::PopupItem).ok();
                    };

                    queue!(
                        stdout,
                        MoveTo((text_offset + x) as u16, (y + i) as u16),
                        Print(truncate(&item, popup_width - 1)),
                        Print(whitespaces(popup_width.saturating_sub(item.len()))),
                        SetAttribute(Attribute::Reset),
                    ).unwrap();
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

        // Draw the command box.
        if let Some((command_box, command_box_input)) = command_box {
            let max_height = 8;
            let y = text_height.saturating_sub(max_height);
            self.draw_command_box(
                &mut stdout,
                command_box,
                command_box_input,
                y,
                text_height - y,
                self.cols
            );
        }

        stdout.flush().unwrap();
    }

    fn draw_command_box(
        &self,
        stdout: &mut std::io::Stdout,
        command_box: &CommandBox,
        command_box_input: &Buffer,
        y: usize,
        height: usize,
        width: usize,
    ) {
        use std::io::Write;
        use crossterm::queue;
        use crossterm::cursor::{self, MoveTo, MoveDown};
        use crossterm::terminal::{Clear, ClearType};
        use crossterm::style::{
            Print, SetForegroundColor, SetBackgroundColor,
            Attribute, SetAttribute
        };

        // The input line.
        THEME.apply(stdout, ThemeItem::CommandBoxPrompt).ok();
        queue!(
            stdout,
            MoveTo(0, y as u16),
            Print("Finder"),
            SetAttribute(Attribute::Reset),
            Print(" "),
            Print(truncate(&command_box_input.text(), width - 7)),
            Clear(ClearType::UntilNewLine),
        ).ok();

        // List items.
        let blank_lines;
        let items_height = height - 1;
        if let Some(r) = command_box.last_response() {
            match &r.body {
                ResponseBody::Preview { items } => {
                    blank_lines = items.len()..items_height;
                    let items = items.iter().enumerate().take(items_height);
                    for (i, item) in items {
                        queue!(
                            stdout,
                            MoveTo(0, (y + i + 1) as u16),
                            Clear(ClearType::CurrentLine),
                            SetAttribute(Attribute::Reset),
                        ).ok();

                        if i == command_box.selected() {
                            queue!(
                                stdout,
                                SetAttribute(Attribute::Bold),
                                SetAttribute(Attribute::Underlined),
                            ).ok();
                        }

                        match &item {
                            PreviewItem::Print { body } => {
                                queue!(
                                    stdout,
                                     Print(truncate(body, width))
                                 ).ok();
                            }
                            PreviewItem::PrintWithFile { file, lineno, body } => {
                                let file_width = min(width, 16);
                                let body_width = width.saturating_sub(file_width);
                                queue!(
                                    stdout,
                                     Print(truncate(&file.display_name, file_width)),
                                     Print(truncate(body, body_width)),
                                 ).ok();
                            }
                        }
                    }
                }
                _ => unreachable!()
            }
        } else {
            blank_lines = 0..items_height;
        }

        // Clear remaining lines.
        for i in blank_lines {
            queue!(
                stdout,
                MoveTo(0, (y + i + 1) as u16),
                Clear(ClearType::CurrentLine),
            ).ok();
        }

        // Move the cursor.
        let cursor_x = match command_box_input.cursors()[0] {
            Cursor::Normal { pos } => pos.x,
            _ => unreachable!(),
        };
        queue!(
            stdout,
            MoveTo((min(7 + cursor_x, width)) as u16, y as u16)
        ).ok();
    }

    fn draw_text_line(
        &mut self,
        stdout: &mut std::io::Stdout,
        buffer: &Buffer,
        line: &ropey::RopeSlice,
        top_left: &TopLeft,
        y: usize,
        text_width: usize,
    ) -> usize {
        use unicode_width::{UnicodeWidthStr, UnicodeWidthChar};
        use crossterm::cursor::{self, MoveTo};
        use crossterm::terminal::{Clear, ClearType};
        use crossterm::style::{
            Print, SetForegroundColor, SetBackgroundColor,
            Attribute, SetAttribute
        };

        let mut n = 0;
        let mut remaining = text_width;
        let mut spans = buffer.highlighted_line(y).iter().peekable();
        let mut current_span = spans.next();
        let mut next_span = spans.peek();
        let mut x = top_left.x;
        let slice = line.slice(top_left.x..);
        let tab_width = buffer.config().tab_width;
        'outer: for mut chunk in slice.chunks() {
            while remaining > 0 && !chunk.is_empty() {
                let mut num_chars = chunk.chars().count();
                match (&current_span, next_span) {
                    (Some(span), _) if span.range.contains(&x) => {
                        num_chars = min(num_chars, span.range.end() + 1 - x);
                        THEME.apply_span(stdout, span.span_type).ok();
                    }
                    (_, Some(span)) if span.range.contains(&x) => {
                        num_chars = min(num_chars, span.range.end() + 1 - x);
                        THEME.apply_span(stdout, span.span_type).ok();
                        current_span = spans.next();
                        next_span = spans.peek();
                    }
                    (Some(span), _) if x < *span.range.start() => {
                        num_chars = span.range.start() - x;
                        queue!(stdout, SetAttribute(Attribute::Reset)).unwrap();
                    }
                    (_, Some(span)) if x < *span.range.start() => {
                        num_chars = span.range.start() - x;
                        queue!(stdout, SetAttribute(Attribute::Reset)).unwrap();
                    }
                    (Some(_), _) => {
                        current_span = spans.next();
                        next_span = spans.peek();
                        queue!(stdout, SetAttribute(Attribute::Reset)).unwrap();
                    }
                    (None, _) => {}
                }

                debug_assert!(num_chars > 0);

                // Truncated # of displayed chars until it fits the display
                // width.
                let index =
                    chunk
                        .char_indices()
                        .nth(num_chars)
                        .map(|(i, _)| i)
                        .unwrap_or_else(|| chunk.len());
                let mut width = chunk[..index]
                    .chars()
                    .enumerate()
                    .fold(0, |sum, (i, ch)| {
                        sum + match ch {
                            '\t' => tab_width - ((x + i) % tab_width),
                            _ => UnicodeWidthChar::width_cjk(ch).unwrap_or(1),
                        }
                    });

                let mut chars_rev = chunk.chars().into_iter().rev();
                while width > remaining {
                    let ch = chars_rev.next().unwrap();
                    width -= UnicodeWidthChar::width_cjk(ch).unwrap_or(1);
                    num_chars -= 1;
                    if num_chars == 0 {
                        break 'outer;
                    }
                }

                let index =
                    chunk
                        .char_indices()
                        .nth(num_chars)
                        .map(|(i, _)| i)
                        .unwrap_or_else(|| chunk.len());
                for(i, ch) in (&chunk[..index]).chars().enumerate() {
                    if ch == '\t' {
                        let n = tab_width - ((x + i) % tab_width);
                        trace!("x=[{}, {}], n={}", x,i,n);
                        queue!(stdout, Print(whitespaces(n))).unwrap();
                    } else {
                        queue!(stdout, Print(ch)).unwrap();
                    }
                }

                chunk = &chunk[min(index, chunk.len())..];
                remaining -= width;
                x += num_chars;
                n += num_chars;
            }
        }

        n
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen).unwrap();
        disable_raw_mode().unwrap();
    }
}
