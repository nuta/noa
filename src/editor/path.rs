use std::{
    cmp::{max, min},
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::{RwLock, RwLockReadGuard};

use crate::fuzzy::FuzzySet;

pub struct PathFinder {
    paths: Arc<RwLock<FuzzySet>>,
    workspace_dir: PathBuf,
}

impl PathFinder {
    pub fn new<T: Into<PathBuf>>(workspace_dir: T) -> PathFinder {
        let paths = Arc::new(RwLock::new(FuzzySet::new()));

        PathFinder {
            paths,
            workspace_dir: workspace_dir.into(),
        }
    }

    pub async fn scan(&self) -> RwLockReadGuard<'_, FuzzySet> {
        scan_paths(&self.workspace_dir, &self.paths).await;
        self.paths.read()
    }
}

async fn scan_paths(workspace_dir: &Path, paths: &Arc<RwLock<FuzzySet>>) {
    use ignore::{WalkBuilder, WalkState};
    WalkBuilder::new(workspace_dir).build_parallel().run(|| {
        Box::new(|dirent| {
            if let Ok(dirent) = dirent {
                let meta = dirent.metadata().unwrap();
                if !meta.is_file() {
                    return WalkState::Continue;
                }
                match dirent.path().to_str() {
                    Some(path) => {
                        let mut extra = 0;

                        // Recently used.
                        if let Ok(atime) = meta.accessed() {
                            if let Ok(elapsed) = atime.elapsed() {
                                extra += (100 / max(1, min(elapsed.as_secs(), 360))) as i64;
                                extra += (100 / max(1, elapsed.as_secs())) as i64;
                            }
                        }

                        // Recently modified.
                        if let Ok(mtime) = meta.modified() {
                            if let Ok(elapsed) = mtime.elapsed() {
                                extra += (10
                                    / max(
                                        1,
                                        min(elapsed.as_secs() / (3600 * 24 * 30), 3600 * 24 * 30),
                                    )) as i64;
                                extra += (100 / max(1, min(elapsed.as_secs(), 360))) as i64;
                                extra += (100 / max(1, elapsed.as_secs())) as i64;
                            }
                        }

                        paths.write().insert(path, extra);
                    }
                    None => {
                        warn!("non-utf8 path: {:?}", dirent.path());
                    }
                }
            }

            WalkState::Continue
        })
    });
}
