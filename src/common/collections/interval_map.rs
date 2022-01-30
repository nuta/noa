use std::ops::Range;

pub struct Entry<I, V> {
    pub interval: Range<I>,
    pub value: V,
}

pub struct IntervalMap<I, V> {
    inner: Vec<Entry<I, V>>,
}

impl<I: PartialOrd + Copy, V> IntervalMap<I, V> {
    pub fn new() -> IntervalMap<I, V> {
        IntervalMap { inner: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn insert(&mut self, interval: Range<I>, value: V) {
        self.inner.push(Entry { interval, value });
    }

    pub fn get_containing(&self, key: I) -> Option<&Entry<I, V>> {
        self.inner
            .iter()
            .find(|e| e.interval.start <= key && key < e.interval.end)
    }

    pub fn iter_overlapping(&self, interval: Range<I>) -> impl Iterator<Item = &Entry<I, V>> {
        let start = interval.start;
        let end = interval.end;
        self.inner
            .iter()
            .filter(move |e| start < e.interval.end && e.interval.start < end)
    }
}
