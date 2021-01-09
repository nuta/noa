use crate::buffer::Buffer;
use crate::rope::{Cursor, Range};
use std::cell::RefCell;
use std::cmp::min;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct TopLeft {
    pub y: usize,
    pub x: usize,
}

impl TopLeft {
    pub fn new(y: usize, x: usize) -> TopLeft {
        TopLeft { y, x }
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

    pub fn goto(&mut self, y: usize, x: usize) {
        let mut buffer = self.buffer.borrow_mut();
        let clamped_y = min(y, buffer.num_lines());
        let clamped_x = min(x, buffer.line_len(clamped_y));
        let cursors = vec![Cursor::new(clamped_y, clamped_x)];
        buffer.set_cursors(cursors);
    }

    pub fn scroll_up(&mut self, y_diff: usize) {
        let mut buffer = self.buffer.borrow_mut();
        buffer.move_cursors(y_diff, 0, 0, 0);
        self.top_left.y = self.top_left.y.saturating_sub(y_diff);
    }

    pub fn scroll_down(&mut self, y_diff: usize) {
        let mut buffer = self.buffer.borrow_mut();
        buffer.move_cursors(0, y_diff, 0, 0);
        self.top_left.y = min(buffer.num_lines(), self.top_left.y + y_diff);
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
