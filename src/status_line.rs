use crate::{
    buffer::Buffer,
    terminal::{Frame, Widget},
};

pub struct StatusLine {}

impl StatusLine {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render(&self, frame: &mut Frame, buffer: &Buffer) {
        let y = frame.height.saturating_sub(2);
        if y < 3 {
            frame.draw_str(y, 0, "too small view");
        }

        frame.fill_reversed(y, 0, frame.width);
    }
}
