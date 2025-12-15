use crate::terminal::{Frame, Widget};

pub struct StatusLine {
}

impl StatusLine {
    pub fn new() -> Self {
        Self {}
    }
}

impl Widget for StatusLine {
    fn render(&self, frame: &mut Frame) {
        frame.draw_str(0, 0, "noa");
    }
}
