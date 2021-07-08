use std::path::{Path, PathBuf};

use ignore::{WalkBuilder, WalkState};

use crate::search_query::SearchQuery;

pub struct PathScanner {
    base_dir: PathBuf,
    include: SearchQuery,
    exclude: SearchQuery,
}

impl PathScanner {
    pub fn new(base_dir: &Path) -> PathScanner {
        PathScanner {
            base_dir: base_dir.to_owned(),
            include: SearchQuery::MatchAll,
            exclude: SearchQuery::MatchNone,
        }
    }

    pub fn scan<F>(&self, callback: Box<F>)
    where
        F: Fn(PathBuf) -> bool + Send + Sync,
    {
        WalkBuilder::new(&self.base_dir).build_parallel().run(|| {
            Box::new(|dirent| {
                if let Ok(dirent) = dirent {
                    let meta = dirent.metadata().unwrap();
                    if !meta.is_file() {
                        return WalkState::Continue;
                    }

                    let path = dirent.path();
                    if callback(path.to_path_buf()) {
                        WalkState::Continue
                    } else {
                        WalkState::Quit
                    }
                } else {
                    WalkState::Continue
                }
            })
        });
    }
}
