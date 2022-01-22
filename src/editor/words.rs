use std::{borrow::Cow, collections::HashMap, ops::Range};

use noa_buffer::raw_buffer::RawBuffer;

use crate::fuzzy::FuzzySet;

pub struct Words {
    fuzzy_set: FuzzySet,
    words_in_lines: Vec<Vec<String>>,
    occurences: HashMap<String, usize>,
}

impl Words {
    pub fn new() -> Words {
        Words {
            fuzzy_set: FuzzySet::new(),
            words_in_lines: Vec::new(),
            occurences: HashMap::new(),
        }
    }

    pub fn query(&self, pattern: &str) -> Vec<(Cow<'_, str>, i64)> {
        self.fuzzy_set.query(pattern)
    }

    pub fn update_line(&mut self, buffer: &RawBuffer, y: usize) {
        // self.fuzzy_set.remove(word)
    }

    pub fn update_lines(&mut self, buffer: &RawBuffer, ys: Range<usize>) {
        for y in ys {
            self.update_line(buffer, y);
        }
    }
}
