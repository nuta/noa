use noa_buffer::buffer::Buffer;

use crate::highlighting::Highlighter;

pub struct DisplayRow {}

pub struct View {
    rows: Vec<DisplayRow>,
    logical_x: usize,
    highlighter: Highlighter,
}

impl View {
    pub fn new(highlighter: Highlighter) -> View {
        View {
            rows: Vec::new(),
            logical_x: 0,
            highlighter,
        }
    }

    pub fn update(&mut self, buffer: &Buffer) {
        self.highlighter.update(buffer);
    }
}
