use std::{borrow::Cow, collections::HashMap, ops::Range};

use futures::StreamExt;
use noa_buffer::raw_buffer::RawBuffer;
use noa_common::collections::fuzzy_set::FuzzySet;

const WORD_MIN_LEN: usize = 4;

pub struct Words {
    words: FuzzySet<()>,
    words_in_lines: Vec<Vec<String>>,
    occurences: HashMap<String, usize>,
}

impl Words {
    pub fn new() -> Words {
        Words {
            words: FuzzySet::new(),
            words_in_lines: Vec::new(),
            occurences: HashMap::new(),
        }
    }

    pub fn new_with_buffer(buffer: &RawBuffer) -> Words {
        let mut words = Words::new();
        words.update_lines(buffer, 0..buffer.num_lines());
        words
    }

    pub fn words(&self) -> &FuzzySet<()> {
        &self.words
    }

    pub fn update_lines(&mut self, buffer: &RawBuffer, ys: Range<usize>) {
        for y in ys {
            self.update_line(buffer, y);
        }
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
                self.words.remove(word);
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
                        self.words.insert(word, (), 0);
                    }

                    word = String::with_capacity(8);
                }
            }
        }
    }
}
