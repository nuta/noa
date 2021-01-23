use std::collections::{BinaryHeap, binary_heap::Iter};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock, RwLockReadGuard};
use crate::editor::Event;
use crate::rope::Point;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub struct WithPriority<P: Ord + Eq, T> {
    pub priority: P,
    pub data: T,
}

impl<P: Ord + Eq, T> WithPriority<P, T> {
    pub fn new(priority: P, data: T) -> WithPriority<P, T> {
        WithPriority {
            priority,
            data,
        }
    }
}

impl<P: Ord + Eq, T> PartialOrd for WithPriority<P, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.priority.cmp(&other.priority))
    }
}

impl<P: Ord + Eq, T> Ord for WithPriority<P, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl<P: Ord + Eq, T> PartialEq for WithPriority<P, T> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl<P: Ord + Eq, T> Eq for WithPriority<P, T> {}

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
    items: Arc<RwLock<BinaryHeap<WithPriority<isize, FinderItem>>>>,
}

impl Finder {
    pub fn new(event_queue: Sender<Event>) -> Finder {
        Finder {
            event_queue,
            selected: 0,
            items: Arc::new(RwLock::new(BinaryHeap::new())),
        }
    }

    pub fn clear(&mut self) {
        self.items.write().unwrap().clear();
        self.selected = 0;
    }

    pub fn items(&self) -> RwLockReadGuard<BinaryHeap<WithPriority<isize, FinderItem>>> {
        self.items.read().unwrap()
    }

    pub fn selected_item(&self) -> Option<FinderItem> {
        let items = self.items.read().unwrap();
        items.iter().nth(self.selected).map(|item| item.data.clone())
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
        self.clear();

        let cwd = std::env::current_dir().unwrap();
        let query2 = query.to_owned();
        let event_queue2 = self.event_queue.clone();
        let items2 = self.items.clone();
        std::thread::spawn(move || {
            provide_file_paths(&query2, items2, &cwd);
            event_queue2.send(Event::Redraw).ok();
        });
    }
}

fn fuzzy_match(query: &str, text: &str) -> Option<isize> {
    sublime_fuzzy::best_match(query, text).map(|m| m.score())
}

fn provide_file_paths(query: &str, items: Arc<RwLock<BinaryHeap<WithPriority<isize, FinderItem>>>>, dir: &Path) {
    const NUM_MATCHES_MAX: usize = 128;
    let mut files = Vec::with_capacity(128);
    let walker = WalkBuilder::new(dir).build();
    for e in walker {
        if let Ok(e) = e {
            let path = e.into_path();
            if let Some(score) = fuzzy_match(query, path.to_str().unwrap()) {
                files.push((score, FinderItem::File {
                    path,
                    pos: None,
                }));

                if files.len() >= NUM_MATCHES_MAX {
                    break;
                }
            }
        }
    }

    for (score, item) in files {
        items.write().unwrap().push(WithPriority::new(score, item));
    }
}
