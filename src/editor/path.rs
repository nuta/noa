use crate::fuzzy::FuzzySet;

pub struct PathFinder {
    paths: FuzzySet,
}

impl PathFinder {
    pub fn new() -> PathFinder {
        PathFinder {
            paths: FuzzySet::new(),
        }
    }

    pub fn paths(&self) -> &FuzzySet {
        &self.paths
    }
}
