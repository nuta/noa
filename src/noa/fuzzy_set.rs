use std::{
    cmp::{max, Ordering},
    collections::BinaryHeap,
};

pub struct FuzzyItem<T> {
    pub score: isize,
    pub value: T,
}

impl<T> PartialEq for FuzzyItem<T> {
    fn eq(&self, other: &FuzzyItem<T>) -> bool {
        self.score.eq(&other.score)
    }
}

impl<T> Eq for FuzzyItem<T> {}

impl<T> PartialOrd for FuzzyItem<T> {
    fn partial_cmp(&self, other: &FuzzyItem<T>) -> Option<Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl<T> Ord for FuzzyItem<T> {
    fn cmp(&self, other: &FuzzyItem<T>) -> Ordering {
        self.score.cmp(&other.score)
    }
}

pub struct FuzzySet<T> {
    capacity: usize,
    items: BinaryHeap<FuzzyItem<T>>,
}

impl<T> FuzzySet<T> {
    pub fn with_capacity(capacity: usize) -> FuzzySet<T> {
        FuzzySet {
            capacity,
            items: BinaryHeap::with_capacity(capacity + 1),
        }
    }

    pub fn push(&mut self, score: isize, value: T) {
        self.items.push(FuzzyItem {
            score: isize::MAX - max(0, score),
            value,
        });

        if self.items.len() > self.capacity {
            self.items.pop();
        }
    }

    pub fn into_vec(self) -> Vec<FuzzyItem<T>> {
        self.items.into_sorted_vec()
    }
}
