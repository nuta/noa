use std::sync::mpsc::Sender;
use std::time::{Instant, Duration};
use std::collections::HashMap;
use std::sync::RwLock;
use lazy_static::lazy_static;
use crate::editor::{Event, EventQueue};
use crate::buffer::{BufferId, Snapshot};
use crate::worker::Job;
use crate::fuzzy::FuzzyVec;

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
}

impl Job for WordCompJob {
    fn execute(&mut self, event_queue: &EventQueue) {
        match CACHES.read().unwrap().get(&self.snapshot.id) {
            Some(cache) if cache.created_at.elapsed() > Duration::from_secs(3) => {
                // Use cached state.
                // cache.
            }
            _ => {
                // Parse the text and fill words.
                let mut cache = WordCompCache {
                    words: FuzzyVec::new(),
                    created_at: Instant::now(),
                };
            }
        }
    }
}
