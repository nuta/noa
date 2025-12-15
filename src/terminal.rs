use crossterm::style::Color;

pub struct Position {
    pub x: u16,
    pub y: u16,
}

pub struct Rect {
    pub top_left: Position,
    pub bottom_right: Position,
}

#[derive(Clone, Copy)]
struct Cell {
    ch: char,
    style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self { ch: ' ', style: Style::default() }
    }
}

#[derive(Clone, Copy)]
struct Style {
    pub fg: Color,
    pub bg: Color,
}

impl Default for Style {
    fn default() -> Self {
        Self { fg: Color::White, bg: Color::Black }
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
        Self { cells, width, height }
    }
}

pub struct FrameView<'a> {
    pub pos: Position,
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
    frames: [Frame; 2],
}

impl Terminal {
    pub fn new(width: usize, height: usize) -> Self {
        let frames = [Frame::new(width, height), Frame::new(width, height)];
        Self { widgets: Vec::new(), active_frame: 0, frames }
    }

    pub fn add_widget(&mut self, rect: Rect, widget: impl Widget + 'static) {
        self.widgets.push(ActiveWidget {
            widget: Box::new(widget),
            rect,
        });
    }

    pub fn render(&self) {
        //
    }
}
