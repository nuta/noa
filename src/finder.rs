use std::sync::mpsc::Sender;
use crate::editor::Event;
use crate::rope::Point;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum FinderItem {
    File {
        path: PathBuf,
        pos: Option<Point>,
    }
}

pub struct Finder {
    event_queue: Sender<Event>,
    selected: usize,
    items: Vec<FinderItem>,
}

impl Finder {
    pub fn new(event_queue: Sender<Event>) -> Finder {
        Finder {
            event_queue,
            selected: 0,
            items: Vec::new(),
        }
    }

    pub fn items(&self) -> &[FinderItem] {
        &self.items
    }

    pub fn selected_item(&self) -> Option<&FinderItem> {
        if self.items.is_empty() {
            return None;
        }

        Some(&self.items[self.selected])
    }

    pub fn move_prev(&mut self) {
        if self.selected == 0 {
            return;
        }

        self.selected -= 1;
    }

    pub fn move_next(&mut self) {
        if self.selected == self.items.len() {
            return;
        }

        self.selected += 1;
    }

    pub fn query(&mut self, query: &str) {

    }
}
