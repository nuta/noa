use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

pub struct FuzzySet {
    matcher: SkimMatcherV2,
    entries: HashSet<String>,
    extra_scores: HashMap<String, i64>,
}

impl FuzzySet {
    pub fn new() -> FuzzySet {
        FuzzySet {
            matcher: SkimMatcherV2::default().smart_case().use_cache(true),
            entries: HashSet::new(),
            extra_scores: HashMap::new(),
        }
    }

    pub fn query(&self, pattern: &str) -> Vec<(Cow<'_, str>, i64)> {
        let mut filtered: Vec<(Cow<'_, str>, i64)> = self
            .entries
            .par_iter()
            .filter_map(|e| {
                self.matcher
                    .fuzzy_match(e, pattern)
                    .map(|score| (Cow::from(e), score + self.extra_scores[e]))
            })
            .collect();

        filtered.sort_by_key(|(_, score)| *score);
        filtered
    }

    pub fn insert<T: Into<String>>(&mut self, entry: T, extra_score: i64) {
        let string = entry.into();
        self.entries.insert(string.clone());
        self.extra_scores.insert(string, extra_score);
    }

    pub fn remove(&mut self, entry: &str) {
        self.entries.remove(entry);
        self.extra_scores.remove(entry);
    }
}