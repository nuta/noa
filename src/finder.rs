use crate::terminal::PromptItem;
use std::collections::BinaryHeap;
use std::path::Path;
use std::rc::Rc;

fn matches(item: &PromptItem, query: &str) -> i32 {
    let mut similarity = 0;
    if query.chars().all(|ch| item.title.contains(ch)) {
        similarity += 10;
    }

    similarity
}

pub struct Finder {
    items: Vec<Rc<PromptItem>>,
    filtered: Vec<Rc<PromptItem>>,
}

impl Finder {
    pub fn new() -> Finder {
        Finder {
            items: Vec::new(),
            filtered: Vec::new(),
        }
    }

    pub fn filtered(&self) -> &[Rc<PromptItem>] {
        &self.filtered
    }

    pub fn reload(&mut self, repo_dir: &Path) {
        self.items.clear();
        for entry in ignore::Walk::new(repo_dir) {
            if let Ok(entry) = entry {
                if entry.metadata().unwrap().is_dir() {
                    continue;
                }

                let title = entry.path().to_str().unwrap().to_owned();
                let item = PromptItem::new('p', PromptItem::PATH_COLOR, title);
                self.items.push(Rc::new(item));
            }
        }

        self.filter("");
    }

    pub fn filter(&mut self, query: &str) {
        let mut heap = BinaryHeap::new();

        for (index, item) in self.items.iter().enumerate() {
            let similarity = matches(item, query);
            if similarity > 0 {
                heap.push((similarity, index));
            }
        }

        self.filtered.clear();
        for (_, index) in heap.iter().take(64) {
            self.filtered.push(self.items[*index].clone());
        }
    }
}
