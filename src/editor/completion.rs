

use parking_lot::{Mutex, MutexGuard};

#[derive(Clone, Copy)]
pub enum CompletionKind {
    CurrentWord,
}

#[derive(Clone)]
pub struct CompletionItem {
    pub kind: CompletionKind,
    pub insert_text: String,
}

pub struct Completion {
    entries: Mutex<Vec<CompletionItem>>,
}

impl Completion {
    pub fn new() -> Completion {
        Completion {
            entries: Mutex::new(Vec::new()),
        }
    }

    pub fn entries(&self) -> MutexGuard<'_, Vec<CompletionItem>> {
        self.entries.lock()
    }

    pub fn clear(&self) {
        self.entries.lock().clear();
    }

    pub fn push(&self, item: CompletionItem) {
        self.entries.lock().push(item);
    }
}
