use std::rc::Rc;
use std::cell::RefCell;
use crate::buffer::Buffer;

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
}
