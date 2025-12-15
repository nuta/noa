use crossterm::style::Color;

pub struct Position {
    pub x: u16,
    pub y: u16,
}

pub struct Rect {
    pub top_left: Position,
    pub bottom_right: Position,
}

struct Cell {
    ch: char,
    style: Style,
}

struct Style {
    pub fg: Color,
    pub bg: Color,
}

struct Frame {
    cells: Vec<Vec<Cell>>,
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
}

impl Terminal {
    pub fn new() -> Self {
        Self { widgets: Vec::new() }
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
