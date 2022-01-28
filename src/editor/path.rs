use std::{
    cmp::{max, min},
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::{RwLock, RwLockReadGuard};

use crate::fuzzy::FuzzySet;

pub async fn scan_paths(workspace_dir: PathBuf) -> FuzzySet {
    let mut paths = RwLock::new(FuzzySet::new());
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

    paths.into_inner()
}
