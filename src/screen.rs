use std::cell::{RefCell, Ref, RefMut};
use std::rc::Rc;
use std::cmp::{min, max};
use crate::file::File;
use crate::fuzzy::FuzzySet;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub const fn new(line: usize, column: usize) -> Position {
        Position { line, column }
    }
}

pub struct View {
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
            top_left: Position::new(0, 0),
            cursor: Position::new(0, 0),
            file,
            needs_redraw: Some(0),
        }
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

pub struct Panel {
    views: Vec<View>,
    current_view_index: usize,
    top_left: Position,
    height: usize,
    width: usize,
}

impl Panel {
    pub fn new(top_left: Position, height: usize, width: usize, views: Vec<View>) -> Panel {
        Panel {
            views,
            current_view_index: 0,
            top_left,
            height,
            width,
        }
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn top_left(&self) -> &Position {
        &self.top_left
    }

    pub fn views(&self) -> &[View] {
        &self.views
    }

    pub fn current_view(&self) -> &View {
        &self.views[self.current_view_index]
    }

    pub fn current_view_mut(&mut self) -> &mut View {
        &mut self.views[self.current_view_index]
    }

    pub fn add_view(&mut self, view: View) {
        self.views.push(view);
        // Make the newly added view active.
        self.current_view_index = self.views.len() - 1;
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Mode {
    Buffer,
    CommandMenu,
}

pub struct TextBox {
    text: String,
    cursor: usize,
}

impl TextBox {
    pub fn new() -> TextBox {
        TextBox {
            text: String::with_capacity(32),
            cursor: 0,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.text.remove(self.cursor - 1);
            self.cursor -= 1;
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub fn move_cursor(&mut self, diff: isize) {
        if diff < 0 {
            self.cursor = self.cursor.saturating_sub(diff.abs() as usize);
        } else {
            self.cursor = max(self.text.len(), self.cursor + diff as usize);
        }
    }
}

pub struct MenuBox {
    textbox: TextBox,
    elements: FuzzySet,
    filtered: Vec<String>,
    selected: usize,
}

impl MenuBox {
    pub fn new() -> MenuBox {
        MenuBox {
            textbox: TextBox::new(),
            elements: FuzzySet::new(),
            filtered: Vec::new(),
            selected: 0,
        }
    }

    pub fn textbox(&self) -> &TextBox {
        &self.textbox
    }

    pub fn textbox_mut(&mut self) -> &mut TextBox {
        &mut self.textbox
    }

    pub fn elements_mut(&mut self) -> &mut FuzzySet {
        &mut self.elements
    }

    pub fn filtered(&self) -> &[String] {
        &self.filtered
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn move_selection(&mut self, diff: isize) {
        if diff < 0 {
            self.selected = self.selected.saturating_sub(diff.abs() as usize);
        } else {
            self.selected = min(self.filtered.len(), self.selected + diff as usize);
        }
    }

    pub fn filter(&mut self) -> &[String] {
        self.filtered = self.elements.search(self.textbox.text());
        self.selected = 0;
        &self.filtered
    }

    pub fn clear(&mut self) {
        self.textbox.clear();
    }

    pub fn enter(&mut self) -> Option<&str> {
        self.clear();
        if self.filtered.len() > 0 {
            Some(&self.filtered[self.selected])
        } else {
            None
        }
    }
}

pub struct Screen {
    mode: Mode,
    width: usize,
    height: usize,
    panels: Vec<Panel>,
    current_panel_index: usize,
    command_menu: MenuBox,
}

impl Screen {
    pub fn new(scratch_view: View, height: usize, width: usize) -> Screen {
        let views = vec![scratch_view];
        let panel = Panel::new(Position::new(0, 0), height, width, views);
        Screen {
            mode: Mode::Buffer,
            width,
            height,
            panels: vec![panel],
            current_panel_index: 0,
            command_menu: MenuBox::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub fn command_menu(&self) -> &MenuBox {
        &self.command_menu
    }

    pub fn command_menu_mut(&mut self) -> &mut MenuBox {
        &mut self.command_menu
    }

    pub fn panels(&self) -> &[Panel] {
        &self.panels
    }

    pub fn current_panel(&self) -> &Panel {
        &self.panels[self.current_panel_index]
    }

    pub fn current_panel_mut(&mut self) -> &mut Panel {
        &mut self.panels[self.current_panel_index]
    }

    pub fn active_view(&self) -> &View {
        self.current_panel().current_view()
    }

    pub fn active_view_mut(&mut self) -> &mut View {
        self.current_panel_mut().current_view_mut()
    }
}
