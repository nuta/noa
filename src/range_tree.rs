use std::cmp::min;
use std::ops;
use std::slice;

use crate::rope::Range;

pub struct Node<V> {
    pub range: Range,
    pub value: V,
}

/// An internval tree to efficiently store buffer text decoration ranges like
/// syntax highlighting. Currently, the internal data structure is not a tree
/// (instead a sorted vector).
pub struct RangeTree<V> {
    nodes: Vec<Node<V>>,
}

impl<V> RangeTree<V> {
    pub fn new() -> RangeTree<V> {
        RangeTree { nodes: Vec::new() }
    }

    /// Updates or inserts `value` with the `range`. `O(max(log n + m, l))`
    /// where `m` is overlapping exisiting nodes and `l` is the number of nodes
    /// after `range`.
    ///
    /// `merge` is used to update an existing node.
    pub fn update_range<F>(&mut self, range: &Range, value: V, merge: F)
    where
        F: Fn(&mut V, &V),
    {
        let mut overlapping_nodes = self.iter_overlapping_mut(range).peekable();
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
            while let Some(node) = overlapping_nodes.next() {
                // node.range
            }
        }
    }

    /// Removes overlapping nodes. `O(n)`.
    pub fn remove_overlapping(&mut self, range: &Range) {
        self.nodes.retain(|node| !node.range.overlaps_with(range));
    }

    /// Returns the iterator of nodes overlapping nodes. `O(log n)`.
    pub fn iter_overlapping(&self, range: &Range) -> slice::Iter<'_, Node<V>> {
        self.nodes[self.overlapping_slice_range(range)].iter()
    }

    /// Returns the iterator of nodes overlapping nodes. `O(log n)`. It's private
    /// since we can't guaranteed that nodes remain sorted (and don't want to
    /// do the whole vector).
    fn iter_overlapping_mut(&mut self, range: &Range) -> slice::IterMut<'_, Node<V>> {
        let slice_range = self.overlapping_slice_range(range);
        self.nodes[slice_range].iter_mut()
    }

    fn overlapping_slice_range(&self, range: &Range) -> ops::Range<usize> {
        let first = self
            .nodes
            .partition_point(|node| node.range < *range && !node.range.overlaps_with(range));
        let search_from = min(first, self.nodes.len());
        let end = search_from
            + self.nodes[search_from..].partition_point(|node| node.range.overlaps_with(range));
        first..end
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
