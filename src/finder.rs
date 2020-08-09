use std::io::Stdout;
use std::cmp::min;
use crate::editor::Editor;
use crate::view::TopLeft;
use crate::editor::Modal;

pub struct FinderModal {
    input: String,
    items: Vec<usize>,
    active_item: usize,
    cursor: usize,
}

impl FinderModal {
    pub fn new() -> FinderModal {
        FinderModal {
            input: String::new(),
            items: Vec::new(),
            active_item: 0,
            cursor: 0,
        }
    }

    fn clamp_active_item(&mut self) {
        self.active_item = min(self.active_item, self.items.len().saturating_sub(1));
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

        // List items.
        for item in &self.items {
        }

        // Move the cursor.
        queue!(
            stdout,
            MoveTo((min(7 + self.cursor, width)) as u16, y as u16)
        ).ok();
    }

    fn move_up(&mut self) {
        self.active_item = self.active_item.saturating_sub(1);
        self.clamp_active_item();
    }

    fn move_down(&mut self) {
        self.active_item += 1;
        self.clamp_active_item();
    }

    fn input(&mut self, editor: &mut Editor, new_text: &str, cursor: usize) {
        self.input = new_text.to_owned();
        self.cursor = cursor;
    }

    fn execute(&mut self, editor: &mut Editor) {

    }
}
