use std::cell::{RefCell, Ref, RefMut};
use std::rc::Rc;
use std::cmp::{min, max};
use crate::buffer::Line;
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
    size: RectSize,
    top_left: Position,
    cursor: Position,
    file: Rc<RefCell<File>>,
    scrolled: bool,
}

impl View {
    pub fn new(file: Rc<RefCell<File>>) -> View {
        View {
            size: RectSize { height: 0, width: 0 },
            top_left: Position::new(0, 0),
            cursor: Position::new(0, 0),
            file,
            scrolled: false,
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

    pub fn resize(&mut self, new_size: RectSize) {
        self.size = new_size;
        self.move_cursor(0, 0);
    }

    pub fn scrolled(&self) -> bool {
        self.scrolled
    }

    pub fn reset_scrolled(&mut self) {
        self.scrolled = false;
    }

    pub fn move_cursor(&mut self, y_diff: isize, x_diff: isize) {
        debug_assert!(
               y_diff.abs() == 1 && x_diff.abs() == 0 // Move vertically.
            || x_diff.abs() == 1 && y_diff.abs() == 0 // Move horizontally.
            || x_diff.abs() == 0 && y_diff.abs() == 0 // Screen resize, etc.
        );

        let file = self.file();
        let buffer = file.buffer();
        let num_lines = buffer.num_lines();
        let mut line = self.cursor.line;
        let mut column = self.cursor.column;

        if x_diff == -1 {
            // Move the cursor left.
            if column == 0 {
                if line > 0 {
                    line -= 1;
                    column = buffer.line_len_at(line);
                }
            } else {
                column -= 1;
            }
        } else if x_diff == 1 {
            // Move the cursor right.
            let line_len = buffer.line_len_at(line);
            if column == line_len {
                if line + 1 < num_lines {
                    line += 1;
                    column = 0;
                }
            } else {
                column += 1;
            }
        }

        if y_diff == -1 {
            // Move the cursor up.
            if line > 0 {
                line -= 1;
            }
        } else if y_diff == 1 {
            // Move the cursor down.
            if line + 1 < num_lines {
                line += 1;
            }
        }

        let column_max = buffer.line_len_at(line);
        drop(buffer);
        drop(file);
        self.cursor.line = line;
        self.cursor.column = min(column, column_max);

        //
        //  Update top_left if necessary (scrolling).
        //
        if self.cursor.line >= self.top_left.line + self.size.height {
            self.top_left.line = self.cursor.line - self.size.height + 1;
            self.scrolled = true;
        }

        if line < self.top_left.line {
            self.top_left.line = self.cursor.line;
            self.scrolled = true;
        }

        if self.cursor.column >= self.top_left.column + self.size.width {
            self.top_left.column = self.cursor.column - self.size.width + 1;
            self.scrolled = true;
        }

        if column < self.top_left.column {
            self.top_left.column = self.cursor.column;
            self.scrolled = true;
        }

        trace!("cursor=({}, {}), top_left=({}, {}), view_size=({}, {})",
            self.cursor.line, self.cursor.column,
            self.top_left.line, self.top_left.column,
            self.size.height, self.size.width);
    }


    pub fn insert(&mut self, ch: char) {
        let cursor = self.cursor;
        let mut file = self.file_mut();
        file.insert(&cursor, ch);
        file.update_highlight(cursor.line);
        drop(file);

        if ch == '\n' {
            self.cursor.line += 1;
            self.cursor.column = 0;
        } else {
            self.cursor.column += 1;
        }

        self.move_cursor(0, 0);
    }

    pub fn backspace(&mut self) {
        let mut cursor = self.cursor;
        if cursor.line == 0 && cursor.column == 0 {
            return;
        }

        let mut file = self.file_mut();
        let prev_len = file.backspace(&cursor);

        if cursor.column == 0 {
            // Move the cursor to the end of previous line.
            cursor.column = prev_len.unwrap();
            cursor.line -= 1;
        } else {
            cursor.column -= 1;
        }

        file.update_highlight(cursor.line);
        drop(file);
        self.move_cursor(0, 0);
        self.cursor = cursor;
    }

    pub fn delete(&mut self) {
        let cursor = self.cursor;
        let mut file = self.file_mut();
        file.delete(&cursor);
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
        self.view.resize(RectSize {
            height: self.height() - 2,
            width: self.width(),
        });
        // TODO: Open a prompt if the file is not yet saved.
    }

    pub fn move_to(&mut self, top_left: Position, size: RectSize) {
        self.top_left = top_left;
        self.size = size;
    }

    pub fn move_cursor(&mut self, y_diff: isize, x_diff: isize) {
        self.view.move_cursor(y_diff, x_diff);
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Mode {
    Buffer,
    Finder,
}

pub struct TextBox {
    text: Line,
    cursor: usize,
}

impl TextBox {
    pub fn new() -> TextBox {
        TextBox {
            text: Line::with_capacity(32),
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
    finder: MenuBox<&'static CommandDefinition>,
    render_all: bool,
}

impl Screen {
    pub fn new(view: View, size: RectSize) -> Screen {
        let panel = Panel::new(Position::new(0, 0), size, view);
        Screen {
            mode: Mode::Buffer,
            size,
            panels: vec![panel],
            current_panel_index: 0,
            finder: MenuBox::new(),
            render_all: true,
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

    pub fn finder(&self) -> &MenuBox<&'static CommandDefinition> {
        &self.finder
    }

    pub fn finder_mut(&mut self) -> &mut MenuBox<&'static CommandDefinition> {
        &mut self.finder
    }

    pub fn panels(&self) -> &[Panel] {
        &self.panels
    }

    pub fn panels_mut(&mut self) -> &mut [Panel] {
        &mut self.panels
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
        // TODO: Support horizontally splitted panels.
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
            // TODO: Support horizontally splitted panels.

            // Calculate new panel size.
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

            // Update its view size.
            panel.view_mut().resize(RectSize {
                height: height - 2 /* including status bar */,
                width,
            });

            trace!("resize panel: #{}, top_left=({}, {}) new_size={}x{}",
                i, 0, x, height, width);
            panel.move_to(Position::new(0, x), RectSize { height, width });
        }

        self.force_render_all();
    }


    /// It is called every time after the frontend rendering has been done.
    pub fn after_rendering(&mut self) {
        self.render_all = false;

        // Reset line modified of all files.
        for panel in self.panels_mut() {
            let view = panel.view_mut();
            view.reset_scrolled();
            view.file_mut().reset_line_modified();
        }
    }

    pub fn render_all(&self) -> bool {
        self.render_all
    }

    /// Forces the frontend to render the entire screen.
    pub fn force_render_all(&mut self) {
        self.render_all = true;
    }
}
