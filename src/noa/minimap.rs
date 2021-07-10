use std::{collections::BTreeMap, ops::Range, path::Path};

use crate::git::{DiffType, Repo};

#[derive(Debug, Clone, Copy)]
pub enum LineStatus {
    AddedLine,
    RemovedLine,
    ModifiedLine,
    Error,
    Warning,
    Cursor,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MiniMapCategory {
    Diff,
    Diagnosis,
    Cursor,
}

pub struct MiniMap {
    maps: BTreeMap<MiniMapCategory, IntervalMap<usize, LineStatus>>,
}

impl MiniMap {
    pub fn new() -> MiniMap {
        let mut maps = BTreeMap::new();
        maps.insert(MiniMapCategory::Diff, IntervalMap::new());
        maps.insert(MiniMapCategory::Cursor, IntervalMap::new());
        maps.insert(MiniMapCategory::Diagnosis, IntervalMap::new());
        MiniMap { maps }
    }

    pub fn insert(&mut self, category: MiniMapCategory, interval: Range<usize>, value: LineStatus) {
        self.maps
            .get_mut(&category)
            .unwrap()
            .insert(interval, value);
    }

    pub fn clear(&mut self, category: MiniMapCategory) {
        self.maps.get_mut(&category).unwrap().clear();
    }

    pub fn get_containing(
        &self,
        category: MiniMapCategory,
        key: usize,
    ) -> Option<&Entry<usize, LineStatus>> {
        self.maps[&category].get_containing(key)
    }

    pub fn iter_overlapping(
        &self,
        category: MiniMapCategory,
        interval: Range<usize>,
    ) -> impl Iterator<Item = &Entry<usize, LineStatus>> {
        self.maps[&category].iter_overlapping(interval)
    }

    pub fn update_git_line_statuses(&mut self, repo: &Repo, buffer_path: &Path, text: &str) {
        let diffs = match repo.compute_line_diffs(buffer_path, text) {
            Ok(diffs) => diffs,
            Err(err) => {
                trace!("failed to get git diff: {:?}", err);
                return;
            }
        };

        self.clear(MiniMapCategory::Diff);
        for diff in diffs {
            trace!(
                "git diff: range={:?}, type={:?}",
                diff.range,
                diff.diff_type
            );

            let value = match diff.diff_type {
                DiffType::Added => LineStatus::AddedLine,
                DiffType::Removed => LineStatus::RemovedLine,
                DiffType::Modified => LineStatus::ModifiedLine,
            };

            self.insert(MiniMapCategory::Diff, diff.range, value);
        }
    }
}

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
