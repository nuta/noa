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

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn selected(&self) -> Option<&T> {
        if self.items.is_empty() {
            return None;
        }

        Some(&self.items[self.selected])
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> impl Iterator<Item = (bool, &T)> {
        let selected = self.selected_index();
        self.items
            .iter()
            .enumerate()
            .map(move |(i, item)| (i == selected, item))
    }

    pub fn select_next(&mut self) {
        self.selected = min(self.items.len().saturating_sub(1), self.selected + 1);
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_last(&mut self) {
        self.selected = self.items.len().saturating_sub(1);
    }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.selected = 0;
    }
}
