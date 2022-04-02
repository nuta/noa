use std::{
    cmp::{max, min},
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use std::{io::ErrorKind, process::Stdio};

use anyhow::Result;

use fuzzy_matcher::FuzzyMatcher;
use noa_buffer::cursor::Position;

use noa_languages::guess_language;
use once_cell::sync::Lazy;
use serde::Deserialize;
use tokio::{io::AsyncBufReadExt, sync::mpsc::UnboundedSender};
use tokio::{io::BufReader, process::Command};

use crate::completion::build_fuzzy_matcher;

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

#[derive(Deserialize)]
#[allow(unused)]
struct RgTextValue {
    text: String,
}

#[derive(Deserialize)]
#[allow(unused)]
struct RgMatchSubmatch {
    #[serde(rename = "match")]
    match_: RgTextValue,
    start: usize,
    end: usize,
}

#[derive(Deserialize)]
#[allow(unused)]
struct RgMatchData {
    path: RgTextValue,
    lines: RgTextValue,
    line_number: usize,
    absolute_offset: usize,
    submatches: Vec<RgMatchSubmatch>,
}

#[derive(Deserialize)]
struct RgMatch {
    #[serde(rename = "type")]
    type_: String,
    data: RgMatchData,
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

    let workspace_dir = workspace_dir.to_owned();
    let query = query.to_owned();
    tokio::spawn(async move {
        let mut cmd = Command::new("rg");
        cmd.args(&[
            "--json",
            "--hidden",
            "--glob",
            "!.git",
            "--no-config",
            "--follow",
            "--max-filesize",
            "8M",
            "--crlf",
        ]);

        if !regex {
            cmd.arg("--fixed-strings");
        }

        if case_insentive {
            cmd.arg("--case-insensitive");
        } else {
            cmd.arg("--smart-case");
        }

        cmd.arg(&query);

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());
        cmd.current_dir(workspace_dir);
        cmd.kill_on_drop(true);

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    notify_warn!("ripgrep is not installed");
                } else {
                    notify_warn!("failed to spawn ripgrep: {}", err);
                }

                return;
            }
        };

        let mut stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(&mut stdout);
        let mut line = String::with_capacity(256);
        let mut preferred = HashMap::new();
        loop {
            line.clear();

            let len = match reader.read_line(&mut line).await {
                Ok(len) => len,
                Err(_) => break,
            };

            if len == 0 || cancel_flag.is_cancelled() {
                break;
            }

            let m = match serde_json::from_str::<RgMatch>(&line) {
                Ok(m) if m.type_ != "match" => continue,
                Ok(m) => m,
                Err(_) => continue,
            };

            let line_text = m.data.lines.text.trim_end().to_owned();
            let byte_range = m
                .data
                .submatches
                .get(0)
                .map(|m| min(m.start, line_text.len())..min(m.end, line_text.len()))
                .unwrap_or(0..0);

            // Prioritize matches that're likely to be definitions.
            //
            // For example, "(struct|type) \1" matches "struct Foo" and "type Foo" in Rust.
            const HEURISTIC_SEARCH_REGEX_EXTRA_SCORE: i64 = 100;
            let path = m.data.path.text;
            let score = if let Some((lang, Some(pattern))) = guess_language(Path::new(&path))
                .map(|lang| (lang, lang.heutristic_search_regex.as_ref()))
            {
                let replaced_pattern = pattern.replace(r"\1", &query);
                preferred
                    .entry(lang.name)
                    .or_insert_with(|| regex::Regex::new(&replaced_pattern).unwrap())
                    .find(&line)
                    .map(|_| HEURISTIC_SEARCH_REGEX_EXTRA_SCORE)
                    .unwrap_or(0)
            } else {
                0
            };

            let _ = tx.send((
                score,
                SearchMatch {
                    path,
                    pos: Position {
                        y: m.data.line_number.saturating_sub(1),
                        x: byte_range.start,
                    },
                    line_text,
                    byte_range,
                },
            ));
        }
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

    WalkBuilder::new(workspace_dir)
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
