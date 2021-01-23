use crate::buffer::Buffer;
use crate::editor::Event;
use crate::rope::Point;
use ignore::WalkBuilder;
use std::collections::{binary_heap::Iter, BinaryHeap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock, RwLockReadGuard};

pub struct WithPriority<P: Ord + Eq, T> {
    pub priority: P,
    pub data: T,
}

impl<P: Ord + Eq, T> WithPriority<P, T> {
    pub fn new(priority: P, data: T) -> WithPriority<P, T> {
        WithPriority { priority, data }
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

const BUFFER_PRIORITY: isize = 10;
const FILE_PRIORITY: isize = 0;

#[derive(Debug, Clone)]
pub enum FinderItem {
    File { path: PathBuf, pos: Option<Point> },
    Buffer { path: PathBuf },
}

pub struct Finder {
    event_queue: Sender<Event>,
    selected: usize,
    items: Arc<RwLock<BinaryHeap<WithPriority<isize, FinderItem>>>>,
    current_query: Option<String>,
}

impl Finder {
    pub fn new(event_queue: Sender<Event>) -> Finder {
        Finder {
            event_queue,
            selected: 0,
            items: Arc::new(RwLock::new(BinaryHeap::new())),
            current_query: None,
        }
    }

    pub fn clear(&mut self) {
        self.items.write().unwrap().clear();
        self.selected = 0;
        self.current_query = None;
    }

    pub fn items(&self) -> RwLockReadGuard<BinaryHeap<WithPriority<isize, FinderItem>>> {
        self.items.read().unwrap()
    }

    pub fn selected_item_index(&self) -> usize {
        self.selected
    }

    pub fn selected_item(&self) -> Option<FinderItem> {
        let items = self.items.read().unwrap();
        items
            .iter()
            .nth(self.selected)
            .map(|item| item.data.clone())
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

    pub fn query(&mut self, query: &str) -> bool {
        if matches!(&self.current_query, Some(current) if current == query) {
            return false;
        }

        self.clear();
        self.current_query = Some(query.to_string());

        if !query.is_empty() {
            let cwd = std::env::current_dir().unwrap();
            let query = query.to_owned();
            let event_queue = self.event_queue.clone();
            let items = self.items.clone();
            std::thread::spawn(move || {
                provide_file_paths(&query, items, &cwd);
                event_queue.send(Event::Redraw).ok();
            });
        }

        true
    }

    pub fn provide_buffer(&mut self, query: &str, buffer: &Buffer) {
        if let Some(path) = buffer.path() {
            if let Some(score) = fuzzy_match(query, path.to_str().unwrap()) {
                self.items.write().unwrap().push(WithPriority::new(
                    score + BUFFER_PRIORITY,
                    FinderItem::Buffer {
                        path: path.to_path_buf(),
                    },
                ));
            }
        }
    }
}

fn fuzzy_match(query: &str, text: &str) -> Option<isize> {
    sublime_fuzzy::best_match(query, text).map(|m| m.score())
}

fn provide_file_paths(
    query: &str,
    items: Arc<RwLock<BinaryHeap<WithPriority<isize, FinderItem>>>>,
    dir: &Path,
) {
    const NUM_MATCHES_MAX: usize = 128;
    let mut files = Vec::with_capacity(128);
    let walker = WalkBuilder::new(dir).build();
    for e in walker {
        if let Ok(e) = e {
            let path = e.into_path();
            if let Some(score) = fuzzy_match(query, path.to_str().unwrap()) {
                files.push((FILE_PRIORITY + score, FinderItem::File { path, pos: None }));

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
