use std::time::{Instant, Duration};
use std::collections::HashMap;
use std::sync::RwLock;
use lazy_static::lazy_static;
use crate::editor::{EventQueue, Event};
use crate::buffer::{BufferId, Snapshot};
use crate::worker::Job;
use crate::fuzzy::FuzzySet;

lazy_static! {
    static ref CACHES: RwLock<HashMap<BufferId, HighlightCache>> = {
        RwLock::new(HashMap::new())
    };
}

pub struct HighlightCache {
    created_at: Instant,
}

pub struct HighlightJob {
    snapshot: Snapshot,
}

impl HighlightJob {
    pub fn new(snapshot: Snapshot) -> HighlightJob {
        HighlightJob {
            snapshot,
        }
    }
}

impl Job for HighlightJob {
    fn execute(&mut self, event_queue: &EventQueue) {
    }
}
