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

    /// Updates or inserts `value` with the `range`. `O(log n + k)` where `k` is
    /// overlapping exisiting nodes.
    ///
    /// `merge` is used to update an existing node.
    pub fn update_range<F>(&mut self, range: &Range, value: V, merge: F)
    where
        F: Fn(&mut V, &V),
    {
        let mut overlapping_nodes = self.iter_overlapping(range).peekable();
        if overlapping_nodes.peek().is_none() {
            // No nodes in the range.
            let pos = self
                .nodes
                .partition_point(|node| node.range < *range && !node.range.overlaps_with(range));
            dbg!(pos);
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

    // Removes overlapping nodes. `O(n)`.
    pub fn remove_overlapping(&mut self, range: &Range) {
        self.nodes.retain(|node| !node.range.overlaps_with(range));
    }

    // Returns the iterator of nodes overlapping nodes. `O(log n)`.
    pub fn iter_overlapping(&self, range: &Range) -> slice::Iter<'_, Node<V>> {
        let first = self
            .nodes
            .partition_point(|node| node.range < *range && !node.range.overlaps_with(range));
        let search_from = min(first, self.nodes.len());
        let end = search_from
            + self.nodes[search_from..].partition_point(|node| node.range.overlaps_with(range));
        self.nodes[first..end].iter()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn insert_nodes_without_overlapping() {
        //  012345678901234567
        //  pub const fn hello
        let mut tree = RangeTree::new();
        tree.update_range(&Range::new(0, 10, 0, 11), "fn", |old, new| {});
        tree.update_range(&Range::new(0, 4, 0, 8), "const", |old, new| {});
        tree.update_range(&Range::new(0, 0, 0, 2), "pub", |old, new| {});
        tree.update_range(&Range::new(0, 13, 0, 17), "hello", |old, new| {});
        assert_eq!(
            tree.iter_overlapping(&Range::new(0, 0, 0, 17))
                .map(|node| node.value)
                .collect::<Vec<&'static str>>(),
            vec!["pub", "const", "fn", "hello"],
        );
        assert_eq!(
            tree.iter_overlapping(&Range::new(0, 1, 0, 17))
                .map(|node| node.value)
                .collect::<Vec<&'static str>>(),
            vec!["pub", "const", "fn", "hello"],
        );
        assert_eq!(
            tree.iter_overlapping(&Range::new(0, 8, 0, 12))
                .map(|node| node.value)
                .collect::<Vec<&'static str>>(),
            vec!["const", "fn"],
        );
    }
}
