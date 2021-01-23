use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock, RwLockReadGuard};
use crate::editor::Event;
use crate::rope::Point;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

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
    items: Arc<RwLock<Vec<FinderItem>>>,
}

impl Finder {
    pub fn new(event_queue: Sender<Event>) -> Finder {
        Finder {
            event_queue,
            selected: 0,
            items: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn clear(&mut self) {
        self.items.write().unwrap().clear();
        self.selected = 0;
    }

    pub fn items(&self) -> RwLockReadGuard<Vec<FinderItem>> {
        self.items.read().unwrap()
    }

    pub fn selected_item(&self) -> Option<FinderItem> {
        let items = self.items.read().unwrap();
        if items.is_empty() {
            return None;
        }

        Some(items[self.selected].clone())
    }

    pub fn move_prev(&mut self) {
        if self.selected == 0 {
            return;
        }

        self.selected -= 1;
    }

    pub fn move_next(&mut self) {
        if self.selected == self.items.read().unwrap().len() {
            return;
        }

        self.selected += 1;
    }

    pub fn query(&mut self, query: &str) {
        let cwd = std::env::current_dir().unwrap();
        let query2 = query.to_owned();
        let event_queue2 = self.event_queue.clone();
        let items2 = self.items.clone();
        std::thread::spawn(move || {
            provide_file_paths(&query2, event_queue2, items2, &cwd);
        });
    }
}

fn provide_file_paths(query: &str, event_queue: Sender<Event>, items: Arc<RwLock<Vec<FinderItem>>>, dir: &Path) {
    const NUM_MATCHES_MAX: usize = 128;
    let mut files = Vec::with_capacity(128);
    let walker = WalkBuilder::new(dir).build();
    for e in walker {
        if let Ok(e) = e {
            let path = e.into_path();
            let display_name = path.to_str().unwrap().to_owned();

            // TODO: fuzzy match
            if display_name.contains(query) {
                files.push(FinderItem::File {
                    path,
                    pos: None,
                });

                if files.len() >= NUM_MATCHES_MAX {
                    break;
                }
            }
        }
    }

    items.write().unwrap().append(&mut files);
}
