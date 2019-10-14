use std::collections::{HashSet, BinaryHeap};

pub struct FuzzySet {
    elements: HashSet<String>,
}

impl FuzzySet {
    pub fn new() -> FuzzySet {
        FuzzySet {
            elements: HashSet::with_capacity(1024),
        }
    }

    pub fn insert(&mut self, value: String) {
        self.elements.insert(value);
    }

    pub fn search<'a>(&'a self, query: &str) -> Vec<String> {
        let mut heap = BinaryHeap::new();
        for elem in &self.elements {
            if let Some(score) = compute_similarity(query, elem) {
                heap.push((score, elem.as_str()));
            }
        }

        let mut results = Vec::with_capacity(heap.len());
        for elem in heap {
            results.push(elem.1.to_owned());
        }

        results
    }
}

fn compute_similarity(query: &str, string: &str) -> Option<i32> {
    let mut iter = string.chars();
    for ch in query.chars() {
        if iter.find(|c| *c == ch).is_none() {
            // unmatch
            return None;
        }
    }

    let mut score = 100;
    if string.starts_with(query) {
        score += 10;
    }

    Some(score)
}
