use std::cmp::min;
use std::ops;
use std::slice;

pub trait Interval: Ord + Clone {
    fn is_empty(&self) -> bool;
    fn includes(&self, other: &Self) -> bool;
    fn overlaps_with(&self, other: &Self) -> bool;
    /// ```text
    /// self:      ...xxxx....
    //  other:     xxxxxxxxxxx
    //  xor:       xxx....xxxx
    //  xor_first: xxx........   <- What this method returns.
    /// ```
    fn xor_first(&self, other: &Self) -> Self;
    /// ```text
    /// self:      ...xxxxx...
    //  other:     xxxxx......
    //  and:       ...xx......   <- What this method returns.
    /// ```
    fn and(&self, other: &Self) -> Self;
    /// ```text
    /// self:      ...xxxxx..
    //  other:     .xxx......
    //            ..xxxxxxx..   <- What this method returns.
    /// ```
    fn merge_adjacent(&self, other: &Self) -> Option<Self>;
}

#[derive(Clone)]
pub struct Node<I: Interval, V: PartialEq + Clone> {
    pub interval: I,
    pub value: V,
}

/// A data structure to efficiently access buffer text decoration (e.g. syntax
/// highlighting) ranges. Currently, the internal data structure is not a tree
/// (instead it's a sorted vector).
pub struct IntervalTree<I: Interval, V: PartialEq + Clone> {
    nodes: Vec<Node<I, V>>,
}

impl<I: Interval, V: PartialEq + Clone> IntervalTree<I, V> {
    pub fn new() -> IntervalTree<I, V> {
        IntervalTree { nodes: Vec::new() }
    }

    /// Updates or inserts `value` with the `range`. `O(max(log n + m, l))`
    /// where `m` is overlapping exisiting nodes and `l` is the number of nodes
    /// after `range`.
    ///
    /// ```text
    /// prev:  ...111..22222..          
    /// range: _^^^^^^^^^_____          
    /// new:   .aabbbaaccddd..          
    /// ```
    pub fn update<F>(&mut self, interval: &I, value: V, update_existing: F)
    where
        F: Fn(&mut V, &V),
    {
        let mut new_intervals = Vec::new();
        let mut overlapping_nodes = self.iter_mut(interval).peekable();
        if overlapping_nodes.peek().is_none() {
            // No overlapping existing nodes.
            new_intervals.push((interval.clone(), value));
        } else {
            for existing_node in overlapping_nodes {
                // Case (a): Insert new nodes between exisiting nodes.
                let xor_first = interval.xor_first(&existing_node.interval);
                if interval.includes(&xor_first) {
                    new_intervals.push((xor_first, value.clone()));
                }

                if interval.includes(&existing_node.interval) {
                    // Case (b): Update a whole exisiting node.
                    update_existing(&mut existing_node.value, &value);
                } else {
                    let overlapping = interval.and(&existing_node.interval);

                    // Case (d): Insert a new node with existing node's value.
                    new_intervals.push((
                        overlapping.xor_first(&existing_node.interval),
                        existing_node.value.clone(),
                    ));

                    // Case (c): Partially update an exisiting node.
                    existing_node.interval = overlapping;
                    update_existing(&mut existing_node.value, &value);
                }
            }
        }

        for (interval, value) in new_intervals {
            let pos = self.nodes.partition_point(|node| {
                node.interval < interval && !node.interval.overlaps_with(&interval)
            });
            self.nodes.insert(pos, Node { interval, value });
        }

        // Merge adjacent nodes with the same value.
        let new_overlapping_nodes = self.overlapping_slice_range(interval);
        let base = new_overlapping_nodes.start;
        let mut iter = self.nodes[new_overlapping_nodes]
            .iter_mut()
            .enumerate()
            .peekable();
        let mut removed = Vec::new();
        while let Some((_, node)) = iter.next() {
            let (next_i, next) = match iter.peek() {
                Some((next_i, next)) => (next_i, next),
                None => break,
            };

            if node.value != next.value {
                continue;
            }

            if let Some(merged_interval) = node.interval.merge_adjacent(&next.interval) {
                node.interval = merged_interval;
                removed.push(base + next_i);
                iter.next();
            }
        }

        // Remove merged nodes.
        for i in removed.iter().rev() {
            self.nodes.remove(*i);
        }
    }

    /// Removes overlapping nodes. `O(n)`.
    pub fn remove(&mut self, interval: &I) {
        self.nodes
            .retain(|node| !node.interval.overlaps_with(interval));
    }

    /// Returns the iterator of nodes overlapping nodes. `O(log n)`.
    pub fn iter(&self, interval: &I) -> slice::Iter<'_, Node<I, V>> {
        let slice_range = self.overlapping_slice_range(interval);
        self.nodes[slice_range].iter()
    }

    /// Returns the iterator of nodes overlapping nodes. `O(log n)`.
    fn iter_mut(&mut self, interval: &I) -> slice::IterMut<'_, Node<I, V>> {
        let slice_range = self.overlapping_slice_range(interval);
        self.nodes[slice_range].iter_mut()
    }

    /// Returns indices of overlapping nodes. `O(log n)`.
    fn overlapping_slice_range(&self, interval: &I) -> ops::Range<usize> {
        let first = self.nodes.partition_point(|node| {
            node.interval < *interval && !node.interval.overlaps_with(interval)
        });
        let search_from = min(first, self.nodes.len());
        let end = search_from
            + self.nodes[search_from..]
                .partition_point(|node| node.interval.overlaps_with(interval));
        first..end
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rope::{Point, Range};

    #[test]
    fn insert_nodes_without_overlapping() {
        //  012345678901234567
        //  pub const fn hello
        let mut tree = IntervalTree::new();
        let update_existing = |old: &mut String, new: &String| {};
        let next_pos = |pos: &Point| Point::new(pos.y, pos.x + 1);
        tree.update(&Range::new(0, 10, 0, 11), "fn".to_owned(), update_existing);
        tree.update(&Range::new(0, 4, 0, 8), "const".to_owned(), update_existing);
        tree.update(&Range::new(0, 0, 0, 2), "pub".to_owned(), update_existing);
        tree.update(
            &Range::new(0, 13, 0, 17),
            "hello".to_owned(),
            update_existing,
        );
        assert_eq!(
            tree.iter(&Range::new(0, 0, 0, 17))
                .map(|node| node.value.to_owned())
                .collect::<Vec<String>>(),
            vec!["pub", "const", "fn", "hello"],
        );
        assert_eq!(
            tree.iter(&Range::new(0, 1, 0, 17))
                .map(|node| node.value.to_owned())
                .collect::<Vec<String>>(),
            vec!["pub", "const", "fn", "hello"],
        );
        assert_eq!(
            tree.iter(&Range::new(0, 8, 0, 12))
                .map(|node| node.value.to_owned())
                .collect::<Vec<String>>(),
            vec!["const", "fn"],
        );
    }
}
