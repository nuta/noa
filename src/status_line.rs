use crate::terminal::{Frame, Widget, Color};

pub struct StatusLine {
}

impl StatusLine {
    pub fn new() -> Self {
        Self {}
    }
}

impl Widget for StatusLine {
    fn render(&self, frame: &mut Frame) {
        let y = frame.height.saturating_sub(1);
        frame.fill_reversed(y, 0, frame.width);
        frame.draw_str(y, 0, "noa");
    }
}
