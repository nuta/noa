use std::collections::HashSet;

use crate::Rope;

#[derive(Clone)]
pub struct Snapshot {
    text: String,
    words: HashSet<String>,
}

const MIN_WORD_LEN: usize = 5;

impl Snapshot {
    pub fn empty() -> Snapshot {
        Snapshot {
            text: String::new(),
            words: HashSet::new(),
        }
    }

    pub fn new(rope: &Rope) -> Snapshot {
        let mut text = String::with_capacity(rope.len_bytes());
        let mut words = HashSet::new();
        let mut current_word = String::with_capacity(16);
        for chunk in rope.chunks() {
            text.push_str(chunk);
            for ch in chunk.chars() {
                if char::is_ascii_alphanumeric(&ch) || ch == '_' {
                    current_word.push(ch);
                } else {
                    if current_word.len() >= MIN_WORD_LEN {
                        words.insert(current_word.clone());
                    }

                    current_word.clear();
                }
            }
        }

        if current_word.len() >= MIN_WORD_LEN {
            words.insert(current_word);
        }

        Snapshot { text, words }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn words(&self) -> impl Iterator<Item = &str> {
        self.words.iter().map(String::as_str)
    }
}
