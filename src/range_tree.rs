use std::cmp::min;
use std::ops;
use std::slice;

use crate::rope::{Point, Range};

pub struct Node<V: PartialEq + Clone> {
    pub range: Range,
    pub value: V,
}

/// A data structure to efficiently access buffer text decoration (e.g. syntax
/// highlighting) ranges. Currently, the internal data structure is not a tree
/// (instead it's a sorted vector).
pub struct RangeTree<V: PartialEq + Clone> {
    nodes: Vec<Node<V>>,
}

impl<V: PartialEq + Clone> RangeTree<V> {
    pub fn new() -> RangeTree<V> {
        RangeTree { nodes: Vec::new() }
    }

    /// Updates or inserts `value` with the `range`. `O(max(log n + m, l))`
    /// where `m` is overlapping exisiting nodes and `l` is the number of nodes
    /// after `range`.
    ///
    /// `merge` is used to update an existing node.
    pub fn update_range<F, N>(&mut self, range: &Range, value: V, merge: F, next_pos: N)
    where
        F: Fn(&mut V, &V),
        N: Fn(&Point) -> Point,
    {
        let slice_range = self.overlapping_slice_range(range);
        if slice_range.is_empty() {
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
            let mut i = slice_range.start;
            let mut num_added = 0;
            let mut range = range.canonicalize();
            while i - num_added < slice_range.end {
                //  range
                // VVVVVVV       (a) Insert new nodes between existing ones.
                //  fn hello     (b) Update the existing node (w/o range update).
                // ^^^^^^^^^     (c) Update the existing node (w/ range update).
                // abbacccdd     (d) Insert the non-overlapping new node (w/o value update).
                //
                let next_node_start = *self.nodes[i].range.front();
                // (a)
                if range.front() < &next_node_start {
                    self.nodes.insert(
                        i,
                        Node {
                            range: Range::from_points(*range.front(), next_node_start),
                            value: value.clone(),
                        },
                    );
                    range.start = next_node_start;
                    i += 1;
                    num_added += 1;
                }

                let existing_node = &mut self.nodes[i];
                let existing_node_back = *existing_node.range.back();
                let old_value = existing_node.value.clone();
                merge(&mut existing_node.value, &value);
                if &existing_node_back > range.back() {
                    // Split a node: (c) & (d)
                    existing_node.range.end = *range.back();
                    let new_node = Node {
                        range: Range::from_points(next_pos(range.back()), existing_node_back),
                        value: value.clone(),
                    };
                    self.nodes.insert(i, new_node);
                    i += 1;
                    num_added += 1;
                }

                range.start = next_pos(&existing_node_back);
                i += 1;
            }
        }
    }

    /// Removes overlapping nodes. `O(n)`.
    pub fn remove_overlapping(&mut self, range: &Range) {
        self.nodes.retain(|node| !node.range.overlaps_with(range));
    }

    /// Returns the iterator of nodes overlapping nodes. `O(log n)`. It's private
    /// since we can't guaranteed that nodes remain sorted (and don't want to
    /// do the whole vector).
    fn iter_overlapping(&self, range: &Range) -> slice::Iter<'_, Node<V>> {
        let slice_range = self.overlapping_slice_range(range);
        self.nodes[slice_range].iter()
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
        let merge = |old: &mut String, new: &String| {};
        let next_pos = |pos: &Point| Point::new(pos.y, pos.x + 1);
        tree.update_range(&Range::new(0, 10, 0, 11), "fn".to_owned(), merge, next_pos);
        tree.update_range(&Range::new(0, 4, 0, 8), "const".to_owned(), merge, next_pos);
        tree.update_range(&Range::new(0, 0, 0, 2), "pub".to_owned(), merge, next_pos);
        tree.update_range(
            &Range::new(0, 13, 0, 17),
            "hello".to_owned(),
            merge,
            next_pos,
        );
        assert_eq!(
            tree.iter_overlapping(&Range::new(0, 0, 0, 17))
                .map(|node| node.value.to_owned())
                .collect::<Vec<String>>(),
            vec!["pub", "const", "fn", "hello"],
        );
        assert_eq!(
            tree.iter_overlapping(&Range::new(0, 1, 0, 17))
                .map(|node| node.value.to_owned())
                .collect::<Vec<String>>(),
            vec!["pub", "const", "fn", "hello"],
        );
        assert_eq!(
            tree.iter_overlapping(&Range::new(0, 8, 0, 12))
                .map(|node| node.value.to_owned())
                .collect::<Vec<String>>(),
            vec!["const", "fn"],
        );
    }
}
