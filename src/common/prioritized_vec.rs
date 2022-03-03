use std::collections::BinaryHeap;

struct Entry<P: Ord, T> {
    priority: P,
    value: T,
}

impl<P: Ord, T> PartialEq for Entry<P, T> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl<P: Ord, T> Eq for Entry<P, T> {}

impl<P: Ord, T> PartialOrd for Entry<P, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.priority.cmp(&other.priority))
    }
}

impl<P: Ord, T> Ord for Entry<P, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

pub struct PrioritizedVec<P: Ord, T> {
    heap: BinaryHeap<Entry<P, T>>,
    max_capacity: usize,
}

impl<P: Ord, T> PrioritizedVec<P, T> {
    pub fn new() -> Self {
        PrioritizedVec {
            heap: BinaryHeap::new(),
            max_capacity: usize::MAX,
        }
    }

    pub fn set_max_capacity(&mut self, max_capacity: usize) {
        self.max_capacity = max_capacity;
    }

    pub fn insert(&mut self, priority: P, value: T) {
        self.heap.push(Entry { priority, value });
        self.remove_to_fit_capacity();
    }

    pub fn extend(&mut self, other: PrioritizedVec<P, T>) {
        self.heap.extend(other.heap);
        self.remove_to_fit_capacity();
    }

    pub fn into_sorted_vec(self) -> Vec<T> {
        self.heap
            .into_sorted_vec()
            .drain(..)
            .map(|entry| entry.value)
            .collect()
    }

    fn remove_to_fit_capacity(&mut self) {
        if self.heap.len() > self.max_capacity {
            self.heap.pop();
        }
    }
}
