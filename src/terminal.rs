use crossterm::style::Color;

pub struct Position {
    pub x: u16,
    pub y: u16,
}

struct Cell {
    pub ch: char,
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

pub struct Terminal {
    widgets: Vec<Box<dyn Widget>>,
}

impl Terminal {
    pub fn new() -> Self {
        Self { widgets: Vec::new() }
    }

    pub fn add_widget(&mut self, widget: impl Widget + 'static) {
        self.widgets.push(Box::new(widget));
    }

    pub fn render(&self) {
        //
    }
}
