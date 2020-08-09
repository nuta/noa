use std::io::Stdout;
use std::cmp::min;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use crate::editor::Editor;
use crate::view::TopLeft;
use crate::editor::Modal;
use crate::terminal::truncate;

enum Item {
    File(PathBuf),
}

pub struct FinderModal {
    input: String,
    items: Vec<Item>,
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

    fn filter(&mut self, dir: &Path) {
        self.items.clear();
        let walker = WalkBuilder::new(dir).build();
        for e in walker {
            if let Ok(e) = e {
                let pathbuf = e.into_path().to_path_buf();
                let string = pathbuf.to_str().unwrap();
                // TODO: fuzzy match
                if string.contains(&self.input) {
                    self.items.push(Item::File(pathbuf));
                }
            }
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
        use crossterm::cursor::{self, MoveTo, MoveDown};
        use crossterm::terminal::{Clear, ClearType};
        use crossterm::style::{
            Print, Color, SetForegroundColor, SetBackgroundColor,
            Attribute, SetAttribute
        };

        // The input line.
        queue!(
            stdout,
            MoveTo(0, y as u16),
            SetBackgroundColor(Color::Magenta),
            Print("Finder"),
            SetAttribute(Attribute::Reset),
            Print(" "),
            Print(truncate(&self.input, width - 7))
        ).ok();

        // List items.
        let items_height = height - 1;
        for (i, item) in self.items.iter().enumerate().take(items_height) {
            queue!(
                stdout,
                MoveTo(0, (y + i + 1) as u16),
                Clear(ClearType::CurrentLine),
                SetAttribute(Attribute::Reset),
            ).ok();

            if i == self.active_item {
                queue!(
                    stdout,
                    SetAttribute(Attribute::Bold),
                    SetAttribute(Attribute::Underlined),
                ).ok();
            }

            match item {
                Item::File(path) => {
                    queue!(
                        stdout,
                        Print(truncate(path.to_str().unwrap(), width))
                    );
                }
            }
        }

        // Clear remaining lines.
        for i in self.items.len()..(items_height) {
            queue!(
                stdout,
                MoveTo(0, (y + i + 1) as u16),
                Clear(ClearType::CurrentLine),
            ).ok();
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
        self.filter(editor.workspace_dir());
    }

    fn execute(&mut self, editor: &mut Editor) {
        if let Some(item) = self.items.get(self.active_item) {
            match item {
                Item::File(path) => {
                    editor.open_file(path);
                }
            }
        }
    }
}
