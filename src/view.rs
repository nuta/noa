use std::rc::Rc;
use std::cell::RefCell;
use crate::buffer::Buffer;
use crate::rope::{Cursor, Range};

pub struct TopLeft {
    pub y: usize,
    pub x: usize,
}

impl TopLeft {
    pub fn new(y: usize, x: usize) -> TopLeft {
        TopLeft {
            y,
            x,
        }
    }
}

pub struct View {
    buffer: Rc<RefCell<Buffer>>,
    top_left: TopLeft,
}

impl View {
    pub fn new(buffer: Rc<RefCell<Buffer>>) -> View {
        View {
            buffer,
            top_left: TopLeft::new(0, 0),
        }
    }

    pub fn buffer(&self) -> &Rc<RefCell<Buffer>> {
        &self.buffer
    }

    pub fn top_left(&self) -> &TopLeft {
        &self.top_left
    }

    pub fn adjust_top_left(&mut self, rows: usize, cols: usize) {
        let buffer = self.buffer.borrow();
        let cursor = &buffer.cursors()[0];
        let pos = match cursor {
            Cursor::Normal { pos, .. } => pos,
            Cursor::Selection(Range { end, .. }) => end,
        };

        // Scroll Up.
        if pos.y < self.top_left.y {
            self.top_left.y = pos.y;
        }

        // Scroll Down.
        if pos.y >= self.top_left.y + rows {
            self.top_left.y = pos.y - rows + 1;
        }

        // Scroll Right.
        if pos.x >= self.top_left.x + cols {
            self.top_left.x = pos.x - cols + 1;
        }

        // Scroll Left.
        if pos.x < self.top_left.x {
            self.top_left.x = pos.x;
        }
    }

    pub fn centering(&mut self, rows: usize) {
        let buffer = self.buffer.borrow();
        if buffer.cursors().len() > 1 {
            return;
        }

        if let Cursor::Normal { pos } = buffer.cursors()[0] {
            self.top_left.y = pos.y.saturating_sub(rows / 2);
        }
    }
}
