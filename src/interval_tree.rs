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
    /// ```
    fn xor(&self, other: &Self) -> (Self, Self);
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

impl<I: Interval + std::fmt::Debug, V: PartialEq + Clone> IntervalTree<I, V> {
    pub fn new() -> IntervalTree<I, V> {
        IntervalTree { nodes: Vec::new() }
    }

    /// Updates or inserts `value` with the `range`.
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
        let mut intervals_between_nodes = Vec::new();
        if overlapping_nodes.peek().is_none() {
            // No overlapping existing nodes.
            new_intervals.push((interval.clone(), value.clone()));
        } else {
            for existing_node in overlapping_nodes {
                let xor_intervals = interval.xor(&existing_node.interval);
                if !xor_intervals.0.is_empty() || !xor_intervals.1.is_empty() {
                    intervals_between_nodes.push(xor_intervals);
                }

                if interval.includes(&existing_node.interval) {
                    // Case (b): Update a whole exisiting node.
                    update_existing(&mut existing_node.value, &value);
                } else {
                    let overlapping = interval.and(&existing_node.interval);

                    // Case (d): Insert a new node with existing node's value.
                    let xs = overlapping.xor(&existing_node.interval);
                    for x in &[xs.0, xs.1] {
                        if !x.is_empty() {
                            new_intervals.push((x.clone(), existing_node.value.clone()));
                        }
                    }

                    // Case (c): Partially update an exisiting node.
                    existing_node.interval = overlapping;
                    update_existing(&mut existing_node.value, &value);
                }
            }
        }

        // Case (a): Insert new nodes between exisiting nodes.
        if !intervals_between_nodes.is_empty() {
            let mut new_intervals2 = vec![
                intervals_between_nodes[0].0.clone(),
                intervals_between_nodes[0].1.clone(),
            ];

            for (b1, b2) in intervals_between_nodes.iter().skip(1) {
                let mut new_intervals3 = Vec::new();
                for a in &new_intervals2 {
                    for &b in &[b1, b2] {
                        if !b.is_empty() {
                            let x = a.and(b);
                            dbg!(&x);
                            if a != b && !x.is_empty() {
                                new_intervals3.push(x);
                            }
                        }
                    }
                }
                dbg!(&new_intervals3);
                new_intervals2 = new_intervals3;
            }

            for a in new_intervals2 {
                if !a.is_empty() && a.overlaps_with(interval) {
                    dbg!(&a);
                    new_intervals.push((a, value.clone()));
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
        let updated_range = new_overlapping_nodes.start.saturating_sub(1)..min(self.nodes.len(), new_overlapping_nodes.end + 1);
        let base = updated_range.start;
        let mut iter = self.nodes[updated_range]
            .iter_mut()
            .enumerate()
            .peekable();
        let mut removed = Vec::new();
        while let Some((i, node)) = iter.next() {
            dbg!(i, iter.peek().is_some());
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

    /// Returns the iterator of all nodes.
    #[cfg(test)]
    pub fn iter_all(&self) -> slice::Iter<'_, Node<I, V>> {
        self.nodes.iter()
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
        tree.update(&Range::new(0, 10, 0, 12), "fn".to_owned(), update_existing);
        tree.update(&Range::new(0, 4, 0, 9), "const".to_owned(), update_existing);
        tree.update(&Range::new(0, 0, 0, 3), "pub".to_owned(), update_existing);
        tree.update(
            &Range::new(0, 13, 0, 17),
            "hello".to_owned(),
            update_existing,
        );
        assert_eq!(
            tree.iter_all()
                .map(|node| node.value.to_owned())
                .collect::<Vec<String>>(),
            vec!["pub", "const", "fn", "hello"],
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

    #[test]
    fn update_existing_nodes1() {
        let mut tree = IntervalTree::new();
        let update_existing = |old: &mut i32, new: &i32| {
            *old |= *new;
        };

        tree.update(&Range::new(0, 2, 0, 5), 0b01, update_existing);
        // current:  ..111..
        // interval: xxxxxxx
        // updated:  2233322
        tree.update(&Range::new(0, 0, 0, 7), 0b10, update_existing);

        assert_eq!(
            tree.iter_all()
                .map(|node| (node.value, node.interval.clone()))
                .collect::<Vec<(i32, Range)>>(),
            vec![
                (
                    0b10,
                    Range {
                        start: Point::new(0, 0),
                        end: Point::new(0, 2),
                    }
                ),
                (
                    0b11,
                    Range {
                        start: Point::new(0, 2),
                        end: Point::new(0, 5),
                    }
                ),
                (
                    0b10,
                    Range {
                        start: Point::new(0, 5),
                        end: Point::new(0, 7),
                    }
                ),
            ]
        );
    }

    #[test]
    fn update_existing_nodes2() {
        let mut tree = IntervalTree::new();
        let update_existing = |old: &mut i32, new: &i32| {
            *old |= *new;
        };

        tree.update(&Range::new(0, 2, 0, 5), 0b01, update_existing);
        tree.update(&Range::new(0, 0, 0, 1), 0b01, update_existing);
        // current:  1.111..
        // interval: xxxxxxx
        // updated:  3233322
        tree.update(&Range::new(0, 0, 0, 7), 0b10, update_existing);

        assert_eq!(
            tree.iter_all()
                .map(|node| (node.value, node.interval.clone()))
                .collect::<Vec<(i32, Range)>>(),
            vec![
                (
                    0b11,
                    Range {
                        start: Point::new(0, 0),
                        end: Point::new(0, 1),
                    }
                ),
                (
                    0b10,
                    Range {
                        start: Point::new(0, 1),
                        end: Point::new(0, 2),
                    }
                ),
                (
                    0b11,
                    Range {
                        start: Point::new(0, 2),
                        end: Point::new(0, 5),
                    }
                ),
                (
                    0b10,
                    Range {
                        start: Point::new(0, 5),
                        end: Point::new(0, 7),
                    }
                ),
            ]
        );
    }

    #[test]
    fn update_existing_nodes3() {
        let mut tree = IntervalTree::new();
        let update_existing = |old: &mut i32, new: &i32| {
            *old |= *new;
        };

        tree.update(&Range::new(0, 0, 0, 4), 0b01, update_existing);
        // current:  1111
        // interval: ..xx
        // updated:  1133
        tree.update(&Range::new(0, 2, 0, 4), 0b10, update_existing);

        assert_eq!(
            tree.iter_all()
                .map(|node| (node.value, node.interval.clone()))
                .collect::<Vec<(i32, Range)>>(),
            vec![
                (
                    0b01,
                    Range {
                        start: Point::new(0, 0),
                        end: Point::new(0, 2),
                    }
                ),
                (
                    0b11,
                    Range {
                        start: Point::new(0, 2),
                        end: Point::new(0, 4),
                    }
                ),
            ]
        );
    }

    #[test]
    fn update_existing_nodes4() {
        let mut tree = IntervalTree::new();
        let update_existing = |old: &mut i32, new: &i32| {
            *old |= *new;
        };

        tree.update(&Range::new(0, 0, 0, 4), 0b01, update_existing);
        // current:  1111
        // interval: xx..
        // updated:  3311
        tree.update(&Range::new(0, 0, 0, 2), 0b10, update_existing);

        assert_eq!(
            tree.iter_all()
                .map(|node| (node.value, node.interval.clone()))
                .collect::<Vec<(i32, Range)>>(),
            vec![
                (
                    0b11,
                    Range {
                        start: Point::new(0, 0),
                        end: Point::new(0, 2),
                    }
                ),
                (
                    0b01,
                    Range {
                        start: Point::new(0, 2),
                        end: Point::new(0, 4),
                    }
                ),
            ]
        );
    }

    #[test]
    fn update_existing_nodes5() {
        let mut tree = IntervalTree::new();
        let update_existing = |old: &mut i32, new: &i32| {
            *old |= *new;
        };
        tree.update(&Range::new(0, 2, 0, 5), 0b01, update_existing);
        tree.update(&Range::new(0, 7, 0, 12), 0b01, update_existing);

        // current:  ..111..11111
        // interval: xxxxxxxxx...
        // updated:  223332233111
        tree.update(&Range::new(0, 0, 0, 9), 0b10, update_existing);

        assert_eq!(
            tree.iter(&Range::new(0, 0, 0, 15))
                .map(|node| (node.value, node.interval.clone()))
                .collect::<Vec<(i32, Range)>>(),
            vec![
                (
                    0b10,
                    Range {
                        start: Point::new(0, 0),
                        end: Point::new(0, 2),
                    }
                ),
                (
                    0b11,
                    Range {
                        start: Point::new(0, 2),
                        end: Point::new(0, 5),
                    }
                ),
                (
                    0b10,
                    Range {
                        start: Point::new(0, 5),
                        end: Point::new(0, 7),
                    }
                ),
                (
                    0b11,
                    Range {
                        start: Point::new(0, 7),
                        end: Point::new(0, 9),
                    }
                ),
                (
                    0b01,
                    Range {
                        start: Point::new(0, 9),
                        end: Point::new(0, 12),
                    }
                ),
            ],
        );
    }


    #[test]
    fn merge_nodes_with_same_value() {
        let mut tree = IntervalTree::new();
        let update_existing = |old: &mut i32, new: &i32| {
            *old |= *new;
        };

        tree.update(&Range::new(0, 1, 0, 2), 0b01, update_existing);
        tree.update(&Range::new(0, 2, 0, 3), 0b11, update_existing);
        tree.update(&Range::new(0, 3, 0, 4), 0b01, update_existing);

        // current:  .131
        // interval: .x..
        // updated:  .331
        tree.update(&Range::new(0, 1, 0, 2), 0b10, update_existing);
        assert_eq!(
            tree.iter_all()
                .map(|node| (node.value, node.interval.clone()))
                .collect::<Vec<(i32, Range)>>(),
            vec![
                (
                    0b11,
                    Range {
                        start: Point::new(0, 1),
                        end: Point::new(0, 3),
                    }
                ),
                (
                    0b01,
                    Range {
                        start: Point::new(0, 3),
                        end: Point::new(0, 4),
                    }
                ),
            ]
        );

        // current:  .331
        // interval: ...x
        // updated:  .333
        tree.update(&Range::new(0, 3, 0, 4), 0b10, update_existing);
        assert_eq!(
            tree.iter_all()
                .map(|node| (node.value, node.interval.clone()))
                .collect::<Vec<(i32, Range)>>(),
            vec![
                (
                    0b11,
                    Range {
                        start: Point::new(0, 1),
                        end: Point::new(0, 4),
                    }
                ),
            ]
        );
    }
}
