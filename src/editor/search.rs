use std::{
    cmp::{max, min},
    collections::{HashMap, HashSet},
    io::{self},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{bail, Result};

use fuzzy_matcher::FuzzyMatcher;
use grep::{
    matcher::Matcher,
    regex::{RegexMatcher, RegexMatcherBuilder},
    searcher::{Searcher, SinkError},
};
use ignore::{WalkBuilder, WalkState};
use noa_buffer::cursor::Position;

use noa_common::oops::OopsExt;

use noa_languages::guess_language;
use once_cell::sync::Lazy;

use regex::Regex;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

use crate::completion::build_fuzzy_matcher;

const FILE_SIZE_MAX: u64 = 8 * 1024 * 1024;
// Avoid using all CPUs: leave some cores to do other tasks.
static NUM_WORKER_CPUS: Lazy<usize> = Lazy::new(|| max(2, num_cpus::get().saturating_sub(2)));

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
    pub byte_range: std::ops::Range<usize>,
}

struct SearchMatchSink<'a> {
    matcher: &'a RegexMatcher,
    tx: &'a UnboundedSender<(i64, SearchMatch)>,
    heutristic_search_caches: HashMap<&'static str, Regex>,
    path: &'a str,
    query: &'a str,
}

impl<'a> SearchMatchSink<'a> {
    fn new(
        matcher: &'a RegexMatcher,
        tx: &'a UnboundedSender<(i64, SearchMatch)>,
        path: &'a str,
        query: &'a str,
    ) -> Self {
        Self {
            matcher,
            tx,
            path,
            heutristic_search_caches: HashMap::new(),
            query,
        }
    }

    fn send(
        &mut self,
        line_text: &str,
        lineno: usize,
        x: usize,
        byte_range: std::ops::Range<usize>,
    ) {
        let score = self.compute_extra_score(line_text);
        let item = SearchMatch {
            path: self.path.to_owned(),
            pos: Position::new(lineno.saturating_sub(1), x),
            line_text: line_text.to_owned(),
            byte_range,
        };

        let _ = self.tx.send((score, item));
    }

    fn compute_extra_score(&mut self, line_text: &str) -> i64 {
        // Prioritize matches that're likely to be definitions.
        //
        // For example, "(struct|type) \1" matches "struct Foo" and "type Foo" in Rust.
        const HEURISTIC_SEARCH_REGEX_EXTRA_SCORE: i64 = 100;
        if let Some((lang, Some(pattern))) = guess_language(Path::new(&self.path))
            .map(|lang| (lang, lang.heutristic_search_regex.as_ref()))
        {
            let replaced_pattern = pattern.replace(r"\1", self.query);
            self.heutristic_search_caches
                .entry(lang.name)
                .or_insert_with(|| regex::Regex::new(&replaced_pattern).unwrap())
                .find(line_text)
                .map(|_| HEURISTIC_SEARCH_REGEX_EXTRA_SCORE)
                .unwrap_or(0)
        } else {
            0
        }
    }
}

impl<'a> grep::searcher::Sink for SearchMatchSink<'a> {
    type Error = io::Error;

    fn matched(
        &mut self,
        _searcher: &grep::searcher::Searcher,
        mat: &grep::searcher::SinkMatch<'_>,
    ) -> Result<bool, io::Error> {
        let lineno = mat.line_number().unwrap() as usize;
        let line_text = match std::str::from_utf8(mat.bytes()) {
            Ok(text) => text.trim_end(),
            Err(err) => {
                return Err(io::Error::error_message(err));
            }
        };

        let line_len = line_text.len();
        let mut start = 0;
        let mut end = 0;
        self.matcher
            .find_iter(mat.bytes(), |m| {
                start = min(m.start(), line_len);
                end = min(m.end(), line_len);
                false
            })
            .unwrap();

        let mut x = 0;
        for (char_i, (byte_i, _)) in line_text.char_indices().enumerate() {
            if byte_i == start {
                x = char_i;
                break;
            }
        }

        self.send(line_text, lineno, x, start..end);
        Ok(true)
    }
}

pub fn search_texts_globally(
    workspace_dir: &Path,
    query: &str,
    tx: UnboundedSender<(i64, SearchMatch)>,
    regex: bool,
    case_insentive: bool,
    cancel_flag: CancelFlag,
) -> Result<()> {
    if query.is_empty() {
        return Ok(());
    }

    let mut builder = RegexMatcherBuilder::new();
    if case_insentive {
        builder.case_insensitive(true);
    } else {
        builder.case_smart(true);
    }

    let matcher = match if regex {
        builder.build(query)
    } else {
        builder.build(&regex::escape(query))
    } {
        Ok(matcher) => matcher,
        Err(err) => {
            bail!("invalid regex: {}", err);
        }
    };

    WalkBuilder::new(workspace_dir)
        .hidden(true)
        .max_filesize(Some(FILE_SIZE_MAX))
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

                let dirent = match dirent {
                    Ok(dirent) => dirent,
                    Err(_) => return WalkState::Continue,
                };

                let path_str = dirent.path().to_str().unwrap().to_owned();
                Searcher::new()
                    .search_path(
                        &matcher,
                        &dirent.path(),
                        SearchMatchSink::new(&matcher, &tx, &path_str, query),
                    )
                    .oops();

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
    WalkBuilder::new(workspace_dir)
        .hidden(true)
        .threads(*NUM_WORKER_CPUS)
        .build_parallel()
        .run(|| {
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
                                            min(
                                                elapsed.as_secs() / (3600 * 24 * 30),
                                                3600 * 24 * 30,
                                            ),
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
    let workspace_dir = workspace_dir.to_path_buf();
    tokio::spawn(async move {
        let mut cmd = Command::new("rg");
        cmd.args(&[
            "--hidden",
            "--glob",
            "!.git",
            "--no-config",
            "--follow",
            "--max-filesize",
            "8M",
            "--crlf",
            "--fixed-strings",
            "--case-sensitive",
            "9z9z9z9z9z9z9z9z9z9z9z9z", /* dummy string that won't match in any files */
        ]);

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());
        cmd.current_dir(workspace_dir);
        cmd.kill_on_drop(true);
        let _ = cmd.status().await;
    });
}
