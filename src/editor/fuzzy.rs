use std::{borrow::Cow, collections::HashSet};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

pub struct FuzzySet {
    matcher: SkimMatcherV2,
    entries: HashSet<String>,
}

impl FuzzySet {
    pub fn new() -> FuzzySet {
        FuzzySet {
            matcher: SkimMatcherV2::default().smart_case().use_cache(true),
            entries: HashSet::new(),
        }
    }

    pub fn query(&self, pattern: &str) -> Vec<(Cow<'_, str>, i64)> {
        let mut filtered: Vec<(Cow<'_, str>, i64)> = self
            .entries
            .par_iter()
            .filter_map(|e| {
                self.matcher
                    .fuzzy_match(e, pattern)
                    .map(|score| (Cow::from(e), score))
            })
            .collect();

        filtered.sort_by_key(|(_, score)| *score);
        filtered
    }

    pub fn insert<T: Into<String>>(&mut self, entry: T) {
        self.entries.insert(entry.into());
    }

    pub fn remove(&mut self, entry: &str) {
        self.entries.remove(entry);
    }
}
