use std::collections::{HashSet, BinaryHeap};
use std::hash::{Hash, Hasher};
use std::cmp::Ordering;

pub trait FuzzySetElement {
    fn as_str(&self) -> &str;
}

struct Element<T: FuzzySetElement>(T);

impl<T: FuzzySetElement> Hash for Element<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl<T: FuzzySetElement> PartialEq for Element<T> {
    fn eq(&self, other: &Element<T>) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl<T: FuzzySetElement> Ord for Element<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.as_str().cmp(other.0.as_str())
    }
}

impl<T: FuzzySetElement> PartialOrd for Element<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.0.as_str().cmp(other.0.as_str()))
    }
}

impl<T: FuzzySetElement> Eq for Element<T> {}

pub struct FuzzySet<T: FuzzySetElement> {
    elements: HashSet<Element<T>>,
}

impl<T: FuzzySetElement> FuzzySet<T> {
    pub fn new() -> FuzzySet<T> {
        FuzzySet {
            elements: HashSet::with_capacity(1024),
        }
    }

    pub fn insert(&mut self, value: T) {
        self.elements.insert(Element(value));
    }

    pub fn search(&self, query: &str) -> Vec<&T> {
        let mut heap = BinaryHeap::new();
        for elem in &self.elements {
            if let Some(score) = compute_similarity(query, elem.0.as_str()) {
                heap.push((score, elem));
            }
        }

        let mut results = Vec::with_capacity(heap.len());
        for elem in heap {
            results.push(&(elem.1).0);
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
