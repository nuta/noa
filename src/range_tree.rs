use std::cmp::min;
use std::slice;

use crate::rope::Range;

pub struct Node<V> {
    pub range: Range,
    pub value: V,
}

pub struct RangeTree<V> {
    nodes: Vec<Node<V>>,
}

impl<V> RangeTree<V> {
    pub fn new() -> RangeTree<V> {
        RangeTree { nodes: Vec::new() }
    }

    pub fn update_range<F>(&mut self, range: &Range, value: V, merge: F)
    where
        F: FnOnce(&mut V, &V),
    {
        let mut overlapping_nodes = self.iter_overlapping(range).peekable();
        if overlapping_nodes.peek().is_none() {
            // No nodes in the range.
            let pos = self
                .nodes
                .partition_point(|node| node.range < *range && !node.range.overlaps_with(range));
            self.nodes.insert(
                pos,
                Node {
                    range: range.clone(),
                    value,
                },
            );
        } else {
            // Needs to overwrite or split the existing nodes.
        }
    }

    pub fn remove_overlapping(&mut self, range: &Range) {}

    pub fn iter_overlapping(&self, range: &Range) -> slice::Iter<'_, Node<V>> {
        let first = self
            .nodes
            .partition_point(|node| node.range < *range && !node.range.overlaps_with(range));
        let end = self.nodes[min(first, self.nodes.len())..]
            .partition_point(|node| node.range.overlaps_with(range));
        self.nodes[first..end].iter()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn insert_nodes() {
        // let mut tree = RangeTree::new();
    }
}
