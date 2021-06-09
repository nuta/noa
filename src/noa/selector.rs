use std::cmp::min;

pub struct Selector<T> {
    selected: usize,
    items: Vec<T>,
}

impl<T> Selector<T> {
    pub fn new() -> Selector<T> {
        Selector {
            selected: 0,
            items: Vec::new(),
        }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> impl Iterator<Item = (bool, &T)> {
        let selected = self.selected();
        self.items
            .iter()
            .enumerate()
            .map(move |(i, item)| (i == selected, item))
            .into_iter()
    }

    pub fn select_next(&mut self) {
        self.selected = min(self.items.len(), self.selected + 1);
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}
