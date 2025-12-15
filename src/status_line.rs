use crate::terminal::{Color, Frame, Widget};

pub struct StatusLine {}

impl StatusLine {
    pub fn new() -> Self {
        Self {}
    }
}

impl Widget for StatusLine {
    fn render(&self, frame: &mut Frame) {
        let y = frame.height.saturating_sub(1);
        frame.fill_reversed(y, 0, frame.width);
    }
}
