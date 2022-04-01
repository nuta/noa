use std::{
    cmp::{max, min},
    collections::HashSet,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{bail, Result};

use fuzzy_matcher::FuzzyMatcher;
use grep::searcher::SinkError;
use noa_buffer::cursor::Position;
use noa_common::oops::OopsExt;

use once_cell::sync::{Lazy};
use tokio::sync::mpsc::UnboundedSender;

use crate::completion::build_fuzzy_matcher;

static NUM_WORKER_CPUS: Lazy<usize> = Lazy::new(|| max(2, num_cpus::get() / 2));

pub struct Utf8Sink<F>(F)
where
    F: FnMut(u64, std::ops::Range<usize>, &str) -> Result<bool, io::Error>;

impl<F> grep::searcher::Sink for Utf8Sink<F>
where
    F: FnMut(u64, std::ops::Range<usize>, &str) -> Result<bool, io::Error>,
{
    type Error = io::Error;

    fn matched(
        &mut self,
        _searcher: &grep::searcher::Searcher,
        mat: &grep::searcher::SinkMatch<'_>,
    ) -> Result<bool, io::Error> {
        let text = match std::str::from_utf8(mat.bytes()) {
            Ok(text) => text,
            Err(err) => {
                return Err(io::Error::error_message(err));
            }
        };
        (self.0)(
            mat.line_number().unwrap(),
            mat.bytes_range_in_buffer(),
            text,
        )
    }
}

#[derive(Clone)]
pub struct CancelFlag {
    cancel_flag: Arc<AtomicBool>,
}

impl CancelFlag {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }
}

#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub path: String,
    pub pos: Position,
    pub line_text: String,
    pub before: std::ops::RangeTo<usize>,
    pub matched: std::ops::Range<usize>,
    pub after: std::ops::RangeFrom<usize>,
}

pub fn search_texts_globally(
    workspace_dir: &Path,
    query: &str,
    tx: UnboundedSender<(i64, SearchMatch)>,
    cancel_flag: CancelFlag,
) -> Result<()> {
    use grep::{matcher::Matcher, regex::RegexMatcherBuilder, searcher::Searcher};
    use ignore::{WalkBuilder, WalkState};

    let query = regex::escape(query);

    let matcher = match RegexMatcherBuilder::new().case_smart(true).build(&query) {
        Ok(matcher) => matcher,
        Err(err) => {
            bail!("invalid regex: {}", err);
        }
    };

    WalkBuilder::new(workspace_dir)
        .threads(*NUM_WORKER_CPUS)
        .build_parallel()
        .run(|| {
            let matcher = matcher.clone();
            let tx = tx.clone();
            let cancel_flag = cancel_flag.clone();
            Box::new(move |dirent| {
                if cancel_flag.is_cancelled() {
                    return WalkState::Quit;
                }

                if let Ok(dirent) = dirent {
                    let meta = dirent.metadata().unwrap();
                    if !meta.is_file() {
                        return WalkState::Continue;
                    }

                    let text = match std::fs::read_to_string(dirent.path()) {
                        Ok(text) => text,
                        Err(err) => {
                            warn!("failed to read {}: {}", dirent.path().display(), err);
                            return WalkState::Continue;
                        }
                    };

                    Searcher::new()
                        .search_slice(
                            &matcher,
                            text.as_bytes(),
                            Utf8Sink(|lineno, _range, line| {
                                matcher
                                    .find_iter(line.as_bytes(), |m| {
                                        let line_text = line.trim_end().to_owned();
                                        let mut x = 0;
                                        for (char_i, (byte_i, _)) in
                                            line_text.char_indices().enumerate()
                                        {
                                            x = char_i;
                                            if m.start() == byte_i {
                                                break;
                                            }
                                        }

                                        let path_str = dirent.path().to_str().unwrap().to_owned();
                                        let m_start = min(m.start(), line_text.len());
                                        let m_end = min(m.end(), line_text.len());
                                        let item = SearchMatch {
                                            path: path_str,
                                            pos: Position::new((lineno as usize) - 1, x),
                                            line_text,
                                            before: ..m_start,
                                            matched: m_start..m_end,
                                            after: m_end..,
                                        };

                                        let _ = tx.send((0, item));

                                        true
                                    })
                                    .oops();

                                Ok(true)
                            }),
                        )
                        .oops();
                }

                WalkState::Continue
            })
        });

    Ok(())
}

pub fn search_paths_globally(
    workspace_dir: &Path,
    query: &str,
    tx: UnboundedSender<(i64, String)>,
    exclude_paths: Option<&HashSet<PathBuf>>,
    cancel_flag: CancelFlag,
) -> Result<()> {
    use ignore::{WalkBuilder, WalkState};

    WalkBuilder::new(workspace_dir).build_parallel().run(|| {
        let matcher = build_fuzzy_matcher();
        let tx = tx.clone();
        let cancel_flag = cancel_flag.clone();
        Box::new(move |dirent| {
            if cancel_flag.is_cancelled() {
                return WalkState::Quit;
            }

            if let Ok(dirent) = dirent {
                let meta = dirent.metadata().unwrap();
                if !meta.is_file() {
                    return WalkState::Continue;
                }

                if let Some(exclude_paths) = exclude_paths.as_ref() {
                    if exclude_paths.contains(dirent.path()) {
                        return WalkState::Continue;
                    }
                }

                match dirent.path().to_str() {
                    Some(path) => {
                        let mut score = match matcher.fuzzy_match(path, query) {
                            Some(score) => score,
                            None => return WalkState::Continue,
                        };

                        // Recently used.
                        if let Ok(atime) = meta.accessed() {
                            if let Ok(elapsed) = atime.elapsed() {
                                score += (100 / max(1, min(elapsed.as_secs(), 360))) as i64;
                                score += (100 / max(1, elapsed.as_secs())) as i64;
                            }
                        }

                        // Recently modified.
                        if let Ok(mtime) = meta.modified() {
                            if let Ok(elapsed) = mtime.elapsed() {
                                score += (10
                                    / max(
                                        1,
                                        min(elapsed.as_secs() / (3600 * 24 * 30), 3600 * 24 * 30),
                                    )) as i64;
                                score += (100 / max(1, min(elapsed.as_secs(), 360))) as i64;
                                score += (100 / max(1, elapsed.as_secs())) as i64;
                            }
                        }

                        let path = path.strip_prefix("./").unwrap_or(path);

                        let _ = tx.send((score, path.to_owned()));
                    }
                    None => {
                        warn!("non-utf8 path: {:?}", dirent.path());
                    }
                }
            }

            WalkState::Continue
        })
    });

    Ok(())
}

/// Reads all files to cache file contents in (kernel) memory.
pub fn warm_up_search_cache(workspace_dir: &Path) {
    use ignore::{WalkBuilder, WalkState};

    WalkBuilder::new(workspace_dir)
        .threads(*NUM_WORKER_CPUS)
        .build_parallel()
        .run(|| {
            let mut buf = vec![0u8; 4096];
            Box::new(move |dirent| {
                if let Ok(dirent) = dirent {
                    let meta = dirent.metadata().unwrap();
                    if !meta.is_file() {
                        return WalkState::Continue;
                    }

                    if let Ok(mut file) = std::fs::File::open(dirent.path()) {
                        let _ = file.read(buf.as_mut_slice());
                    }
                }

                WalkState::Continue
            })
        });
}
