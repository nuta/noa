use std::io::Write;

pub use crossterm::event::Event;
pub use crossterm::event::KeyCode;
pub use crossterm::style::Attribute;
use crossterm::style::Attributes;
pub use crossterm::style::Color;

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

    pub fn resize(&mut self, width: u16, height: u16) {
        let mut new_cells = vec![Cell::default(); width as usize * height as usize];

        // Copy the old cells to the new cells, preserving the position
        // on the screen.
        for y in 0..height {
            if y < self.height {
                for x in 0..width {
                    if x < self.width {
                        let y = y as usize;
                        let x = x as usize;
                        let new_w = width as usize;
                        let old_w = self.width as usize;
                        new_cells[y * new_w + x] = self.cells[y * old_w + x];
                    }
                }
            }
        }

        self.width = width;
        self.height = height;
    }

    pub fn draw_str(&mut self, y: u16, x: u16, text: &str) {
        let width = self.width as usize;
        let y_base = y as usize;
        let x_base = x as usize;
        for (i, ch) in text.chars().enumerate() {
            self.cells[y_base * width + x_base + i].ch = ch;
        }
    }

    pub fn fill_reversed(&mut self, y: u16, x: u16, width: u16) {
        let width = width as usize;
        let y_base = y as usize;
        let x_base = x as usize;
        for i in 0..width {
            self.cells[y_base * width + x_base + i]
                .style
                .attrs
                .set(Attribute::Reverse);
        }
    }
}

pub trait Widget {
    fn render(&self, frame: &mut Frame);
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

    pub fn wait_for_event(&self) -> Result<Event, std::io::Error> {
        crossterm::event::read()
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
    for y in 0..height {
        for x in 0..width {
            let old_cell = active_frame.cells[y * width + x];
            let new_cell = standby_frame.cells[y * width + x];
            if old_cell != new_cell {
                let x_u16: u16 = x.try_into().unwrap();
                let y_u16: u16 = y.try_into().unwrap();

                queue!(std::io::stdout(), MoveTo(x_u16, y_u16)).expect("failed to move cursor");

                if old_cell.style != new_cell.style {
                    queue!(
                        std::io::stdout(),
                        SetBackgroundColor(new_cell.style.bg),
                        SetForegroundColor(new_cell.style.fg),
                        SetAttributes(new_cell.style.attrs),
                    )
                    .expect("failed to move cursor");
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
