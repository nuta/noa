pub use crossterm::event::Event;
pub use crossterm::event::KeyCode;
pub use crossterm::style::Color;

#[derive(Clone, Copy)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Copy)]
pub struct Rect {
    pub top_left: Position,
    pub bottom_right: Position,
}

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
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fg: Color::White,
            bg: Color::Black,
        }
    }
}

struct Frame {
    cells: Vec<Cell>,
    width: usize,
    height: usize,
}

impl Frame {
    pub fn new(width: usize, height: usize) -> Self {
        let cells = vec![Cell::default(); width * height];
        Self {
            cells,
            width,
            height,
        }
    }
}

pub struct FrameView<'a> {
    pub rect: &'a Rect,
    frame: &'a mut Frame,
}

pub trait Widget {
    fn render(&self, view: &mut FrameView<'_>);
}

struct ActiveWidget {
    widget: Box<dyn Widget>,
    rect: Rect,
}

pub struct Terminal {
    widgets: Vec<ActiveWidget>,
    active_frame: usize,
    frames: (Frame, Frame),
}

impl Terminal {
    pub fn new(width: usize, height: usize) -> Self {
        initialize_terminal();
        let frames = (Frame::new(width, height), Frame::new(width, height));
        Self {
            widgets: Vec::new(),
            active_frame: 0,
            frames,
        }
    }

    pub fn add_widget(&mut self, rect: Rect, widget: impl Widget + 'static) {
        self.widgets.push(ActiveWidget {
            widget: Box::new(widget),
            rect,
        });
    }

    pub fn wait_for_event(&self) -> Result<Event, std::io::Error> {
        crossterm::event::read()
    }

    pub fn render(&mut self) {
        let (active_frame, standby_frame) = if self.active_frame == 0 {
            (&mut self.frames.0, &mut self.frames.1)
        } else {
            (&mut self.frames.1, &mut self.frames.0)
        };

        for ActiveWidget { widget, rect } in &self.widgets {
            widget.render(&mut FrameView {
                frame: standby_frame,
                rect,
            });
        }

        render_diff(active_frame, standby_frame);
    }
}

fn render_diff(active_frame: &mut Frame, standby_frame: &mut Frame) {
    use crossterm::cursor::MoveTo;
    use crossterm::queue;
    use crossterm::style::Print;
    use crossterm::style::SetBackgroundColor;
    use crossterm::style::SetForegroundColor;

    for y in 0..active_frame.height {
        for x in 0..active_frame.width {
            let old_cell = active_frame.cells[y * active_frame.width + x];
            let new_cell = standby_frame.cells[y * standby_frame.width + x];
            if old_cell != new_cell {
                let x_u16: u16 = x.try_into().unwrap();
                let y_u16: u16 = y.try_into().unwrap();
                queue!(std::io::stdout(), MoveTo(x_u16, y_u16),).expect("failed to move cursor");

                if old_cell.style != new_cell.style {
                    queue!(
                        std::io::stdout(),
                        SetBackgroundColor(new_cell.style.bg),
                        SetForegroundColor(new_cell.style.fg),
                    )
                    .expect("failed to move cursor");
                }

                queue!(std::io::stdout(), Print(new_cell.ch),).expect("failed to print cell");
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
