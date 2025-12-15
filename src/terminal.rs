use std::io::Write;
use std::time::Duration;

pub use crossterm::event::Event;
pub use crossterm::event::KeyCode;
pub use crossterm::style::Attribute;
use crossterm::style::Attributes;
pub use crossterm::style::Color;

use crate::display_width::DisplayWidth;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Cell {
    ch: char,
    style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: Style::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Style {
    pub fg: Color,
    pub bg: Color,
    pub attrs: Attributes,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fg: Color::Reset,
            bg: Color::Reset,
            attrs: Attributes::none(),
        }
    }
}

pub struct Frame {
    cells: Vec<Cell>,
    pub width: u16,
    pub height: u16,
}

impl Frame {
    fn new(width: u16, height: u16) -> Self {
        let cells = vec![Cell::default(); width as usize * height as usize];
        Self {
            cells,
            width,
            height,
        }
    }

    fn get_mut(&mut self, y: u16, x: u16, char_width: u16) -> Option<&mut Cell> {
        if y >= self.height || x + char_width > self.width {
            debug!("out of bounds: ({y}, {x} + {char_width})");
            None
        } else {
            let y_usize = y as usize;
            let x_usize = x as usize;
            let width_usize = self.width as usize;
            Some(&mut self.cells[y_usize * width_usize + x_usize])
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        let mut new_frame = Frame::new(width, height);

        // Copy the old cells to the new cells, preserving the position
        // on the screen.
        for y in 0..height {
            if y < self.height {
                for x in 0..width {
                    if x < self.width {
                        let y_usize = y as usize;
                        let x_usize = x as usize;
                        let new_w = width as usize;
                        let old_w = self.width as usize;
                        let new_index = y_usize * new_w + x_usize;
                        let old_index = y_usize * old_w + x_usize;
                        new_frame.cells[new_index] = self.cells[old_index];
                    }
                }
            }
        }

        *self = new_frame;
    }

    pub fn draw_str(&mut self, y: u16, mut x: u16, text: &str) {
        for ch in text.chars() {
            let width = ch.display_width();
            if let Some(cell) = self.get_mut(y, x, width) {
                cell.ch = ch;
            }

            x += width;
        }
    }

    pub fn fill_reversed(&mut self, y: u16, x: u16, n: u16) {
        for i in 0..n {
            if let Some(cell) = self.get_mut(y, x + i, 1) {
                cell.style.attrs.set(Attribute::Reverse);
            }
        }
    }
}

pub struct Terminal {
    active_index: u8,
    frames: (Frame, Frame),
}

impl Terminal {
    pub fn new() -> Self {
        let (width, height) = crossterm::terminal::size().expect("failed to get terminal size");

        initialize_terminal();

        let frames = (Frame::new(width, height), Frame::new(width, height));
        Self {
            active_index: 0,
            frames,
        }
    }

    pub fn wait_for_events(&self) -> Result<Vec<Event>, std::io::Error> {
        let mut events = Vec::with_capacity(8);

        // Blocking read the first event.
        events.push(crossterm::event::read()?);

        // Non-blocking read all pending events.
        for _ in 0..32 {
            let readable = crossterm::event::poll(Duration::from_millis(0))?;
            if !readable {
                break;
            }

            events.push(crossterm::event::read()?);
        }

        Ok(events)
    }

    pub fn frame(&mut self) -> &mut Frame {
        if self.active_index == 0 {
            &mut self.frames.1
        } else {
            &mut self.frames.0
        }
    }

    pub fn flush(&mut self) {
        let (active_frame, standby_frame) = if self.active_index == 0 {
            (&mut self.frames.0, &mut self.frames.1)
        } else {
            (&mut self.frames.1, &mut self.frames.0)
        };

        render_diff(active_frame, standby_frame);

        std::mem::swap(active_frame, standby_frame);
        std::io::stdout().flush().expect("failed to flush stdout");
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.frames.0.resize(width, height);
        self.frames.1.resize(width, height);
    }
}

fn render_diff(active_frame: &mut Frame, standby_frame: &mut Frame) {
    use crossterm::cursor::MoveTo;
    use crossterm::queue;
    use crossterm::style::Print;
    use crossterm::style::SetAttributes;
    use crossterm::style::SetBackgroundColor;
    use crossterm::style::SetForegroundColor;

    let width = active_frame.width as usize;
    let height = active_frame.height as usize;
    let mut current_style: Option<Style> = None;
    for y in 0..height {
        for x in 0..width {
            let old_cell = active_frame.cells[y * width + x];
            let new_cell = standby_frame.cells[y * width + x];
            let style_changed = current_style != Some(new_cell.style);
            if old_cell.ch != new_cell.ch || style_changed {
                let x_u16: u16 = x.try_into().unwrap();
                let y_u16: u16 = y.try_into().unwrap();

                queue!(std::io::stdout(), MoveTo(x_u16, y_u16)).expect("failed to move cursor");

                if style_changed {
                    queue!(
                        std::io::stdout(),
                        SetBackgroundColor(new_cell.style.bg),
                        SetForegroundColor(new_cell.style.fg),
                        SetAttributes(new_cell.style.attrs),
                    )
                    .expect("failed to move cursor");
                    current_style = Some(new_cell.style);
                }

                queue!(std::io::stdout(), Print(new_cell.ch)).expect("failed to print cell");
            }
        }
    }
}

fn initialize_terminal() {
    use crossterm::event::EnableBracketedPaste;
    use crossterm::execute;
    use crossterm::terminal::Clear;
    use crossterm::terminal::ClearType;
    use crossterm::terminal::EnterAlternateScreen;
    use crossterm::terminal::enable_raw_mode;

    enable_raw_mode().expect("failed to enable raw mode");
    execute!(
        std::io::stdout(),
        EnterAlternateScreen,
        EnableBracketedPaste,
        Clear(ClearType::All)
    )
    .expect("failed to enable events");

    std::panic::set_hook(Box::new(|info| {
        restore_terminal();
        eprintln!("panic: {info}");
        error!("panic: {info}");
        std::process::exit(1);
    }));
}

fn restore_terminal() {
    use crossterm::event::DisableBracketedPaste;
    use crossterm::execute;
    use crossterm::terminal::LeaveAlternateScreen;
    use crossterm::terminal::disable_raw_mode;

    execute!(
        std::io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen
    )
    .expect("failed to disable events");
    disable_raw_mode().expect("failed to disable raw mode");
}

impl Drop for Terminal {
    fn drop(&mut self) {
        restore_terminal();
    }
}
