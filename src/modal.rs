use std::io::Stdout;
use std::cmp::min;
use crate::editor::Editor;
use crate::view::TopLeft;

pub trait Modal {
    fn draw(&self, stdout: &mut Stdout, y: usize, height: usize, width: usize);
    fn move_up(&mut self);
    fn move_down(&mut self);
    fn input(&mut self, editor: &mut Editor, new_text: &str, cursor: usize);
    fn execute(&mut self, editor: &mut Editor);
    }

pub struct FinderModal {
    input: String,
    cursor: usize,
}

impl FinderModal {
    pub fn new() -> FinderModal {
        FinderModal {
            input: String::new(),
            cursor: 0,
        }
    }
}

impl Modal for FinderModal {
    fn draw(&self, stdout: &mut Stdout, y: usize, height: usize, width: usize) {
        use std::io::Write;
        use crossterm::queue;
        use crossterm::cursor::{self, MoveTo};
        use crossterm::terminal::{Clear, ClearType};
        use crossterm::style::{
            Print, Color, SetForegroundColor, SetBackgroundColor,
            Attribute, SetAttribute
        };

        trace!("y={}, h={}, w={}", y, height, width);
        // The input line.
        queue!(
            stdout,
            MoveTo(0, y as u16),
            SetBackgroundColor(Color::Magenta),
            Print("Finder"),
            SetAttribute(Attribute::Reset),
            Print(" "),
            Print(&self.input[..min(self.input.len(), width - 7)])
        ).ok();

        // Move the cursor.
        queue!(
            stdout,
            MoveTo((min(7 + self.cursor, width)) as u16, y as u16)
        ).ok();
    }

    fn move_up(&mut self) {

    }

    fn move_down(&mut self) {

    }

    fn input(&mut self, editor: &mut Editor, new_text: &str, cursor: usize) {
        self.input = new_text.to_owned();
        self.cursor = cursor;
    }

    fn execute(&mut self, editor: &mut Editor) {

    }
}
