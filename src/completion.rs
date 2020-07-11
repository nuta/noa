use std::sync::mpsc::Sender;
use std::time::{Instant, Duration};
use std::collections::HashMap;
use std::sync::RwLock;
use lazy_static::lazy_static;
use crate::editor::{Event, EventQueue};
use crate::buffer::{BufferId, Snapshot};
use crate::worker::Job;
use crate::fuzzy::FuzzyVec;

const NUM_COMP_ITEMS: usize = 5;
const MIN_WORD_LEN: usize = 5;

lazy_static! {
    static ref CACHES: RwLock<HashMap<BufferId, WordCompCache>> = {
        RwLock::new(HashMap::new())
    };
}

pub struct WordCompCache {
    words: FuzzyVec,
    created_at: Instant,
}

pub struct WordCompJob {
    snapshot: Snapshot,
}

impl WordCompJob {
    pub fn new(snapshot: Snapshot) -> WordCompJob {
        WordCompJob {
            snapshot,
        }
    }

    pub fn parse(&self) -> FuzzyVec {
        let mut words = FuzzyVec::new();
        let mut current_word = String::new();
        for ch in &self.snapshot.buf {
            if char::is_ascii_alphanumeric(&ch) || ch == '_' {
                current_word.push(ch);
            } else {
                if current_word.len() >= MIN_WORD_LEN {
                    words.append(current_word.clone());
                }

                current_word.clear();
            }
        }

        words
    }
}

impl Job for WordCompJob {
    fn execute(&mut self, event_queue: &EventQueue) {
        let current_word = ""; // TODO:
        if current_word.len() < 3 {
            return;
        }

        let needs_update = match CACHES.read().unwrap().get(&self.snapshot.id) {
            None => true,
            Some(cache) if cache.created_at.elapsed() > Duration::from_secs(3) => true,
            _ => false,
        };

        if needs_update {
            // Update the cache entry.
            let words = self.parse();
            CACHES.write().unwrap().insert(self.snapshot.id, WordCompCache {
                words,
                created_at: Instant::now(),
            });
        }

        // Fiter by the current word.
        let filtered: Vec<String> = CACHES.read().unwrap()
            .get(&self.snapshot.id).unwrap()
            .words.search(current_word, NUM_COMP_ITEMS)
            .iter().map(|s| s.to_string())
            .collect();
    }
}
