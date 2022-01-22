use std::{borrow::Cow, collections::HashMap, ops::Range};

use futures::StreamExt;
use noa_buffer::raw_buffer::RawBuffer;

use crate::fuzzy::FuzzySet;

const WORD_MIN_LEN: usize = 4;

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
        for word in self
            .words_in_lines
            .get(y)
            .map(|v| v.iter())
            .unwrap_or_else(|| [].iter())
        {
            let n = self.occurences.get_mut(word).unwrap();
            *n -= 1;
            if *n == 0 {
                self.fuzzy_set.remove(word);
                self.occurences.remove(word);
            }
        }

        let mut word = String::with_capacity(8);
        for chunk in buffer.rope().line(y).chunks() {
            for c in chunk.chars() {
                if c == '_'
                    || c.is_ascii_alphabetic()
                    || (!word.is_empty() && c.is_ascii_digit() || c == '-')
                {
                    word.push(c);
                } else if !word.is_empty() {
                    if word.len() >= WORD_MIN_LEN {
                        self.fuzzy_set.insert(word);
                    }

                    word = String::with_capacity(8);
                }
            }
        }
    }

    pub fn update_lines(&mut self, buffer: &RawBuffer, ys: Range<usize>) {
        for y in ys {
            self.update_line(buffer, y);
        }
    }
}
