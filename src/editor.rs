use std::rc::Rc;
use std::cell::RefCell;
use crate::buffer::Buffer;


pub struct Editor {
    buffers: Vec<Rc<RefCell<Buffer>>>,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            buffers: vec![Rc::new(RefCell::new(Buffer::new()))],
        }
    }

     pub fn run(&mut self) {
     }
}
