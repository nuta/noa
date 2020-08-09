use crate::editor::Editor;
use crate::view::TopLeft;

pub trait Modal {
    fn draw(&self, top_left: &TopLeft, rows: usize, cols: usize);
    fn move_up(&mut self);
    fn move_down(&mut self);
    fn input(&mut self, editor: &mut Editor, new_text: &str);
    fn execute(&mut self, editor: &mut Editor);
    }

pub struct FinderModal {
}

impl FinderModal {
    pub fn new() -> FinderModal {
        FinderModal {
        }
    }
}

impl Modal for FinderModal {
    fn draw(&self, top_left: &TopLeft, rows: usize, cols: usize) {

    }

    fn move_up(&mut self) {

    }

    fn move_down(&mut self) {

    }

    fn input(&mut self, editor: &mut Editor, new_text: &str) {

    }

    fn execute(&mut self, editor: &mut Editor) {

    }
}
