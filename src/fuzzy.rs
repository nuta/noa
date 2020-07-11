///
/// A ordered `Vec` which supports fuzzy search.
///
#[derive(Clone)]
pub struct FuzzyVec {
    /// The *unordered* array of a haystack.
    entries: Vec<String>,
}

impl FuzzyVec {
    /// Creates a `FuzzyVec`.
    pub fn new() -> FuzzyVec {
        FuzzyVec {
            entries: Vec::new(),
        }
    }

    /// Creates a `FuzzyVec` with the given capacity.
    pub fn with_capacity(cap: usize) -> FuzzyVec {
        FuzzyVec {
            entries: Vec::with_capacity(cap),
        }
    }

    pub fn iter(&self) -> std::slice::Iter<String> {
        self.entries.iter()
    }

    /// Returns the number of entiries.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    // Clears the contents.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the nth entry from the end of the entries.
    pub fn nth_last(&self, nth: usize) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        self.entries.get(self.entries.len() - nth - 1).map(|s| s.to_owned())
    }

    /// appends a entry.
    pub fn append(&mut self, entry: String) {
        self.entries.push(entry);
    }

    /// Searches entiries for `query` in a fuzzy way and returns the result
    /// ordered by the similarity.
    pub fn search(&self, query: &str) -> Vec<&str> {
        fuzzy_search(&self.entries, query)
    }

    /// Sorts the entries by string values.
    pub fn sort(&mut self) {
        self.entries.sort_by(|a, b| a.cmp(b));
    }
}

/// Searches `entiries` for `query` in *fuzzy* way and returns the result
/// ordered by the similarity.
fn fuzzy_search<'a>(
    entries: &'a [String],
    query: &str
) -> Vec<&'a str> {
    if query.is_empty() {
        // Return the all entries.
        return entries.iter().map(|s| s.as_str()).collect();
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
    let mut filtered = entries
        .iter()
        .filter(|s| is_fuzzily_matched(s, query))
        .map(|s| s.as_str())
        .collect::<Vec<_>>();
    filtered.sort_by_cached_key(|entry| compute_score(entry, query));
    filtered
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
