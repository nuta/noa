use std::collections::HashSet;

///
/// A ordered `Vec` which supports fuzzy search.
///
#[derive(Clone)]
pub struct FuzzySet {
    /// The *unordered* array of a haystack.
    entries: HashSet<String>,
}

impl FuzzySet {
    /// Creates a `FuzzySet`.
    pub fn new() -> FuzzySet {
        FuzzySet {
            entries: HashSet::new(),
        }
    }

    pub fn entries(&self) -> &HashSet<String> {
        &self.entries
    }

    /// appends a entry.
    pub fn append(&mut self, entry: String) {
        self.entries.insert(entry);
    }

    /// Searches entiries for `query` in a fuzzy way and returns the result
    /// ordered by the similarity.
    pub fn search(&self, query: &str, max_num: usize) -> Vec<&str> {
        fuzzy_search(&self.entries, query, max_num)
    }
}

/// Searches `entiries` for `query` in *fuzzy* way and returns the result
/// ordered by the similarity.
fn fuzzy_search<'a>(entries: &'a HashSet<String>, query: &str, max_num: usize) -> Vec<&'a str> {
    if query.is_empty() {
        // Return the all entries.
        return vec![];
    }

    /// Check if entries contain the query characters with correct order.
    fn is_fuzzily_matched(s: &str, query: &str) -> bool {
        let mut iter = s.chars();
        for q in query.chars() {
            loop {
                match iter.next() {
                    None => return false,
                    Some(c) if c == q => break,
                    Some(_) => {}
                }
            }
        }
        true
    }

    // Filter entries by the query.
    let mut filtered: Vec<&str> = entries
        .iter()
        .filter(|s| is_fuzzily_matched(s, query))
        .map(|s| s.as_str())
        .collect();

    filtered.sort_by_cached_key(|entry| compute_score(entry, query));
    filtered
        .iter()
        .take(max_num)
        .copied()
        .collect::<Vec<&str>>()
}

/// Computes the similarity. Lower is more similar.
fn compute_score(entry: &str, query: &str) -> u8 {
    let mut score = std::u8::MAX;

    if entry == query {
        score -= 100;
    }

    if entry.starts_with(query) {
        score -= 10;
    }

    score
}
