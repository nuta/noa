use std::collections::BinaryHeap;

pub struct Entry<T> {
    pub priority: isize,
    pub value: T,
}

impl<T> PartialOrd for Entry<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Entry<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.priority.cmp(&self.priority)
    }
}

impl<T> PartialEq for Entry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl<T> Eq for Entry<T> {}

pub struct PrioritizedVec<T> {
    heap: BinaryHeap<Entry<T>>,
    capacity: usize,
}

impl<T> PrioritizedVec<T> {
    pub fn new(capacity: usize) -> PrioritizedVec<T> {
        PrioritizedVec {
            heap: BinaryHeap::new(),
            capacity,
        }
    }

    pub fn insert(&mut self, priority: isize, value: T) {
        self.heap.push(Entry { priority, value });

        while self.heap.len() >= self.capacity {
            self.heap.pop();
        }
    }

    pub fn into_vec(self) -> Vec<Entry<T>> {
        self.heap.into_sorted_vec()
    }
}
