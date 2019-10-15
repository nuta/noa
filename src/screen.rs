use std::cell::{RefCell, Ref, RefMut};
use std::rc::Rc;
use std::cmp::{min, max};
use crate::buffer::IString;
use crate::editor::CommandDefinition;
use crate::file::File;
use crate::fuzzy::{FuzzySet, FuzzySetElement};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub const fn new(line: usize, column: usize) -> Position {
        Position { line, column }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RectSize {
    pub height: usize,
    pub width: usize,
}

#[derive(Clone)]
pub struct View {
    top_left: Position,
    cursor: Position,
    file: Rc<RefCell<File>>,
}

impl View {
    pub fn new(file: Rc<RefCell<File>>) -> View {
        View {
            top_left: Position::new(0, 0),
            cursor: Position::new(0, 0),
            file,
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
        let cursor = self.cursor;
        let mut file = self.file_mut();
        file.buffer_mut().insert(&cursor, ch);
        file.update_highlight(cursor.line);
        drop(file);

        if ch == '\n' {
            self.cursor.line += 1;
            self.cursor.column = 0;
        } else {
            self.cursor.column += 1;
        }
    }

    pub fn backspace(&mut self) {
        let mut cursor = self.cursor;
        if cursor.line == 0 && cursor.column == 0 {
            return;
        }

        let mut file = self.file_mut();
        let prev_len = file.buffer_mut().backspace(&cursor);
        file.update_highlight(cursor.line);
        drop(file);

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
        let cursor = self.cursor;
        let mut file = self.file_mut();
        file.buffer_mut().delete(&cursor);
        file.update_highlight(cursor.line);
    }
}

pub struct Panel {
    view: View,
    top_left: Position,
    size: RectSize,
}

impl Panel {
    pub fn new(top_left: Position, size: RectSize, view: View) -> Panel {
        Panel {
            view,
            top_left,
            size,
        }
    }

    pub fn height(&self) -> usize {
        self.size.height
    }

    pub fn width(&self) -> usize {
        self.size.width
    }

    pub fn top_left(&self) -> &Position {
        &self.top_left
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
        // TODO: Open a prompt if the file is not yet saved.
    }

    pub fn move_to(&mut self, top_left: Position, size: RectSize) {
        self.top_left = top_left;
        self.size = size;
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Mode {
    Buffer,
    CommandMenu,
}

pub struct TextBox {
    text: IString,
    cursor: usize,
}

impl TextBox {
    pub fn new() -> TextBox {
        TextBox {
            text: IString::with_capacity(32),
            cursor: 0,
        }
    }

    pub fn text(&self) -> &str {
        self.text.as_str()
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

// TODO: Remove `Clone` requirement.
pub struct MenuBox<T: FuzzySetElement + Clone> {
    textbox: TextBox,
    elements: FuzzySet<T>,
    filtered: Vec<T>,
    selected: usize,
}

impl<T: FuzzySetElement + Clone> MenuBox<T> {
    pub fn new() -> MenuBox<T> {
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

    pub fn elements_mut(&mut self) -> &mut FuzzySet<T> {
        &mut self.elements
    }

    pub fn filtered(&self) -> &[T] {
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

    pub fn filter(&mut self) {
        self.filtered = self.elements.search(self.textbox.text())
            .iter().map(|e| (*e).clone()).collect();
        self.selected = 0;
    }

    pub fn clear(&mut self) {
        self.textbox.clear();
    }

    pub fn enter(&mut self) -> Option<&T> {
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
    size: RectSize,
    panels: Vec<Panel>,
    current_panel_index: usize,
    command_menu: MenuBox<&'static CommandDefinition>,
}

impl Screen {
    pub fn new(view: View, size: RectSize) -> Screen {
        let panel = Panel::new(Position::new(0, 0), size, view);
        Screen {
            mode: Mode::Buffer,
            size,
            panels: vec![panel],
            current_panel_index: 0,
            command_menu: MenuBox::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.size.width
    }

    pub fn height(&self) -> usize {
        self.size.height
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub fn command_menu(&self) -> &MenuBox<&'static CommandDefinition> {
        &self.command_menu
    }

    pub fn command_menu_mut(&mut self) -> &mut MenuBox<&'static CommandDefinition> {
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
        self.current_panel().view()
    }

    pub fn active_view_mut(&mut self) -> &mut View {
        self.current_panel_mut().view_mut()
    }

    pub fn move_panel(&mut self, diff: isize) {
        // TODO:
        assert!(diff.abs() == 1);

        self.current_panel_index = if diff < 0 {
            if self.current_panel_index == 0 {
                self.panels.len() - 1
            } else {
                self.current_panel_index - 1
            }
        } else {
            if self.current_panel_index == self.panels.len() - 1 {
                0
            } else {
                self.current_panel_index + 1
            }
        };
    }

    pub fn split_vertically(&mut self) {
        // TODO: Support vertically splitted panels.
        // Fill fields with zero and run resize() to divide the screen width
        // equally.
        let view = self.current_panel().view();
        let size = RectSize {
            height: self.current_panel().height(),
            width: 0,
        };
        let panel_left = Panel::new(Position::new(0, 0), size, view.clone());
        let panel_right = Panel::new(Position::new(0, 0), size, view.clone());
        self.panels.remove(self.current_panel_index);
        self.panels.push(panel_left);
        self.panels.push(panel_right);
        self.resize(self.size);

        self.current_panel_index = self.panels.len() - 1;
    }

    pub fn resize(&mut self, new_size: RectSize) {
        trace!("resize: new_size={:?}", new_size);
        let old_height = self.size.height;
        self.size = new_size;

        let mut remaining_width = new_size.width;
        let num_panels = self.panels.len();
        for (i, panel) in self.panels.iter_mut().enumerate() {
            // TODO: Support vertically splitted panels.
            let top_left = panel.top_left();
            assert!(top_left.line == 0 && panel.height() == old_height);

            let is_last_panel = i == num_panels - 1;
            let x = new_size.width - remaining_width;
            let height = new_size.height;
            let width = if is_last_panel {
                remaining_width
            } else {
                let new_width = new_size.width / num_panels;
                remaining_width -= new_width;
                new_width
            };

            trace!("resize panel: #{}, top_left=({}, {}) new_size={}x{}",
                i, 0, x, height, width);
            panel.move_to(Position::new(0, x), RectSize { height, width });
        }
    }
}
