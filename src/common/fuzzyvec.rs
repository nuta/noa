use std::collections::{HashMap, HashSet};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use parking_lot::Mutex;
use rayon::prelude::*;

#[derive(Clone)]
struct Entry<T: Sync + Clone> {
    value: T,
    extra_score: isize,
}

#[derive(Clone)]
pub struct Items<T: Sync + Clone> {
    keys: HashSet<String>,
    entries: HashMap<String, Entry<T>>,
}

impl<T: Sync + Clone> Items<T> {
    pub fn query(&self, matcher: &SkimMatcherV2, pattern: &str) -> Vec<(&str, &T, isize)> {
        let mut filtered: Vec<(&str, &T, isize)> = self
            .keys
            .par_iter()
            .filter_map(|key| {
                let entry = &self.entries[key];
                if pattern.is_empty() {
                    return Some((key.as_str(), &entry.value, entry.extra_score));
                }

                matcher.fuzzy_match(key, pattern).map(|score| {
                    (
                        key.as_str(),
                        &entry.value,
                        (score as isize) + entry.extra_score,
                    )
                })
            })
            .collect();

        filtered.sort_by_key(|(_, _, score)| *score);
        filtered
    }

    fn insert<K: Into<String>>(&mut self, key: K, value: T, extra_score: isize) {
        let key = key.into();
        self.keys.insert(key.clone());
        self.entries.insert(key, Entry { value, extra_score });
    }

    fn remove(&mut self, key: &str) {
        self.keys.remove(key);
        self.entries.remove(key);
    }

    fn clear(&mut self) {
        self.keys.clear();
        self.entries.clear();
    }
}

pub struct FuzzyVec<T: Sync + Clone> {
    matcher: SkimMatcherV2,
    items: Mutex<Items<T>>,
    top_entries: Mutex<Vec<(String, T)>>,
}

impl<T: Sync + Clone> FuzzyVec<T> {
    pub fn new() -> FuzzyVec<T> {
        let matcher = SkimMatcherV2::default().smart_case().use_cache(true);
        FuzzyVec {
            matcher,
            top_entries: Mutex::new(Vec::new()),
            items: Mutex::new(Items {
                keys: HashSet::new(),
                entries: HashMap::new(),
            }),
        }
    }

    pub fn insert<K: Into<String>>(&self, key: K, value: T, extra_score: isize) {
        self.items.lock().insert(key, value, extra_score);
    }

    pub fn clear(&self) {
        self.items.lock().clear();
        self.top_entries.lock().clear();
    }

    pub fn remove(&self, key: &str) {
        self.items.lock().remove(key);
    }

    pub fn filter_by_key(&self, pattern: &str) {
        let mut entries = Vec::new();
        for (key, value, _) in self.items.lock().query(&self.matcher, pattern) {
            entries.push((key.to_owned(), value.clone()));
        }
        *self.top_entries.lock() = entries;
    }

    pub fn top_entries(&self) -> Vec<(String, T)> {
        self.top_entries.lock().clone()
    }
}

impl<T: Sync + Clone> Default for FuzzyVec<T> {
    fn default() -> Self {
        Self::new()
    }
}
