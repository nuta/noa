use std::cell::{RefCell, Ref, RefMut};
use std::rc::Rc;
use std::cmp::min;
use crate::file::File;
use crate::layout::Position;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Mode {
    Buffer,
}

pub struct View {
    mode: Mode,
    top_left: Position,
    cursor: Position,
    file: Rc<RefCell<File>>,
    /// It contains the y-axis offset from `top_left.line` from which the frontnend
    /// needs to redraw.
    needs_redraw: Option<usize>,
}

impl View {
    pub fn new(file: Rc<RefCell<File>>) -> View {
        View {
            mode: Mode::Buffer,
            top_left: Position::new(0, 0),
            cursor: Position::new(0, 0),
            file,
            needs_redraw: Some(0),
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn top_left(&self) -> &Position {
        &self.top_left
    }

    pub fn cursor(&self) -> &Position {
        &self.cursor
    }

    pub fn file<'a>(&'a self) -> Ref<'a, File> {
        self.file.borrow()
    }

    pub fn file_mut<'a>(&'a mut self) -> RefMut<'a, File> {
        self.file.borrow_mut()
    }

    pub fn mark_as_drawed(&mut self) {
        self.needs_redraw = None;
    }

    pub fn move_cursor(&mut self, y_diff: isize, x_diff: isize) {
        let mut line = self.cursor.line;
        let num_lines = self.file().buffer().num_lines();

        // Update the x-axis.
        let mut column = self.cursor.column;
        if x_diff < 0 {
            // Move the cursor left.
            let mut diff = x_diff.abs() as usize;
            if column < diff && line > 0 {
                while diff > 0 && line > 0 {
                    let line_len = self.file().buffer().line_len_at(line - 1);
                    if diff < line_len {
                        line -= 1;
                        column = line_len - diff + 1;
                        break;
                    }

                    diff -= line_len + 1;
                    line -= 1;
                }
            } else {
                column = column.saturating_sub(diff);
            }
        } else {
            // Move the cursor right.
            let mut diff = x_diff as usize;
            let line_len = self.file().buffer().line_len_at(line);
            if column + diff > line_len && line + 1 < num_lines {
                while diff > 0 && line + 1 < num_lines {
                    let line_len = self.file().buffer().line_len_at(line + 1);
                    if diff < line_len {
                        line += 1;
                        column = diff - 1;
                        break;
                    }

                    diff -= line_len + 1;
                    line += 1;
                }
            } else {
                column += x_diff as usize;
            }
        }

        // Update the y-axis.
        if y_diff < 0 {
            line = line.saturating_sub(y_diff.abs() as usize);
        } else {
            line += y_diff as usize;
        }

        self.cursor.line = min(line, num_lines - 1);
        let new_line_num_columns =
            self.file().buffer().line_len_at(self.cursor.line);
        self.cursor.column = min(column, new_line_num_columns);
    }


    pub fn insert(&mut self, ch: char) {
        let cursor = self.cursor.clone();
        self.file_mut().buffer_mut().insert(&cursor, ch);

        self.needs_redraw = match self.needs_redraw {
            Some(current) => Some(std::cmp::min(current, cursor.line)),
            None => Some(cursor.line),
        };

        if ch == '\n' {
            self.cursor.line += 1;
            self.cursor.column = 0;
        } else {
            self.cursor.column += 1;
        }
    }

    pub fn backspace(&mut self) {
        let mut cursor = self.cursor.clone();
        if cursor.line == 0 && cursor.column == 0 {
            return;
        }

        let prev_len = self.file_mut().buffer_mut().backspace(&cursor);

        if cursor.column == 0 {
            // Move the cursor to the end of previous line.
            cursor.column = prev_len.unwrap();
            cursor.line -= 1;
        } else {
            cursor.column -= 1;
        }

        self.cursor = cursor;
    }

    pub fn delete(&mut self) {
        let cursor = self.cursor.clone();
        self.file_mut().buffer_mut().delete(&cursor);
    }
}
