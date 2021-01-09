use crate::buffer::{BufferId, Snapshot};
use crate::editor::{Event, EventQueue};
use crate::fuzzy::FuzzySet;
use crate::worker::Job;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

const NUM_COMP_ITEMS: usize = 5;
const MIN_WORD_LEN: usize = 5;
const CURRENT_WORD_MIN_LEN: usize = 2;

lazy_static! {
    static ref CACHES: RwLock<HashMap<BufferId, WordCompCache>> = RwLock::new(HashMap::new());
}

pub struct WordCompCache {
    words: FuzzySet,
    created_at: Instant,
}

pub struct WordCompJob {
    snapshot: Snapshot,
}

impl WordCompJob {
    pub fn new(snapshot: Snapshot) -> WordCompJob {
        WordCompJob { snapshot }
    }

    pub fn parse(&self) -> FuzzySet {
        let mut words = FuzzySet::new();
        let mut current_word = String::new();
        for ch in self.snapshot.buf.chars() {
            if char::is_ascii_alphanumeric(&ch) || ch == '_' {
                current_word.push(ch);
            } else {
                if current_word.len() >= MIN_WORD_LEN {
                    words.append(current_word.clone());
                }

                current_word.clear();
            }
        }

        if current_word.len() >= MIN_WORD_LEN {
            words.append(current_word);
        }

        words
    }
}

impl Job for WordCompJob {
    fn execute(&mut self, event_queue: &EventQueue) {
        let current_word = match self.snapshot.main_cursor {
            Some(pos) => match self.snapshot.buf.word_at(&pos) {
                Some((_, word)) => word,
                None => return,
            },
            _ => {
                event_queue.enqueue(Event::NoCompletion);
                return;
            }
        };

        if current_word.len() < CURRENT_WORD_MIN_LEN {
            event_queue.enqueue(Event::NoCompletion);
            return;
        }

        let needs_update = match CACHES.read().unwrap().get(&self.snapshot.buffer_id) {
            None => true,
            Some(cache) if cache.created_at.elapsed() > Duration::from_secs(3) => true,
            _ => false,
        };

        if needs_update {
            // Update the cache entry.
            let words = self.parse();
            CACHES.write().unwrap().insert(
                self.snapshot.buffer_id,
                WordCompCache {
                    words,
                    created_at: Instant::now(),
                },
            );
        }

        // Fiter by the current word.
        let mut filtered = FuzzySet::new();
        let lock = CACHES.read().unwrap();
        let iter = lock
            .get(&self.snapshot.buffer_id)
            .unwrap()
            .words
            .search(&current_word, NUM_COMP_ITEMS);
        for s in iter {
            filtered.append(s.to_string());
        }

        event_queue.enqueue(Event::Completion {
            buffer_id: self.snapshot.buffer_id,
            items: filtered,
        });
    }
}
