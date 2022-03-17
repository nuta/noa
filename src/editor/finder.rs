use std::{
    cmp::{max, min},
    collections::HashSet,
    io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::{bail, Result};

use fuzzy_matcher::FuzzyMatcher;
use grep::searcher::SinkError;
use noa_buffer::cursor::Position;
use noa_common::{oops::OopsExt, prioritized_vec::PrioritizedVec};
use noa_compositor::Compositor;

use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{
    actions::{execute_action_or_notify, ACTIONS},
    completion::build_fuzzy_matcher,
    document::{open_file, DocumentId},
    editor::Editor,
    ui::selector_view::{SelectorContent, SelectorItem, SelectorView},
};

#[derive(Clone, Debug)]
enum FinderItem {
    File(String),
    Buffer {
        name: String,
        id: DocumentId,
    },
    SearchMatch {
        path: String,
        pos: Position,
        line_text: String,
        before: std::ops::RangeTo<usize>,
        matched: std::ops::Range<usize>,
        after: std::ops::RangeFrom<usize>,
    },
    Action {
        name: String,
    },
}

pub fn open_finder(compositor: &mut Compositor<Editor>, editor: &mut Editor) {
    let selector: &mut SelectorView = compositor.get_mut_surface_by_name("selector");
    selector.open("finder", true, Some(Box::new(update_items)));
    update_items(editor, "");
}

fn select_item(compositor: &mut Compositor<Editor>, editor: &mut Editor, item: FinderItem) {
    info!("selected item: {:?}", item);
    match item {
        FinderItem::Buffer { id, .. } => {
            editor.documents.switch_by_id(id);
        }
        FinderItem::File(path) => {
            let path = Path::new(&path);
            match editor.documents.switch_by_path(path) {
                Some(_) => {}
                None => match open_file(compositor, editor, path, None) {
                    Ok(id) => {
                        editor.documents.switch_by_id(id);
                    }
                    Err(err) => {
                        notify_anyhow_error!(err);
                    }
                },
            }
        }
        FinderItem::SearchMatch { path, pos, .. } => {
            let path = Path::new(&path);
            match editor.documents.switch_by_path(path) {
                Some(_) => {}
                None => match open_file(compositor, editor, path, Some(pos)) {
                    Ok(id) => {
                        editor.documents.switch_by_id(id);
                    }
                    Err(err) => {
                        notify_anyhow_error!(err);
                    }
                },
            }
        }
        FinderItem::Action { name } => {
            execute_action_or_notify(editor, compositor, &name);
        }
    }
}

fn update_items(editor: &mut Editor, query: &str) {
    let workspace_dir = editor.workspace_dir.clone();

    let mut items = PrioritizedVec::new();
    let mut visited_paths = HashSet::new();

    if let Some(query) = query.strip_prefix('>') {
        // Actions.
        let matcher = build_fuzzy_matcher();
        for action in ACTIONS {
            if let Some(score) = matcher.fuzzy_match(action.name(), query) {
                items.insert(
                    score,
                    FinderItem::Action {
                        name: action.name().to_owned(),
                    },
                );
            }
        }
    } else if !query.starts_with('/') {
        // Buffers.
        let matcher = build_fuzzy_matcher();
        for (id, doc) in editor.documents.documents().iter() {
            if let Some(score) = matcher.fuzzy_match(doc.path_in_str(), query) {
                items.insert(
                    score + 100,
                    FinderItem::Buffer {
                        name: doc.name().to_owned(),
                        id: *id,
                    },
                );
            }

            visited_paths.insert(doc.path().to_owned());
        }
    }

    let (items_tx, mut items_rx) = unbounded_channel();
    let query = query.to_owned();
    // Search file contents or paths.
    tokio::task::spawn_blocking(move || {
        if let Some(query) = query.strip_prefix('/') {
            if query.len() < 4 {
                notify_warn!("too short query");
                return;
            }

            if let Err(err) = search_globally(&workspace_dir, query, items_tx) {
                notify_warn!("failed to search globally: {}", err);
            }
        } else {
            search_paths(&workspace_dir, &query, items_tx, &visited_paths);
        }
    });

    editor.jobs.await_in_mainloop(
        async move {
            while let Some((priority, item)) = items_rx.recv().await {
                items.insert(priority, item);
            }
            Ok(items.into_sorted_vec())
        },
        |_editor, compositor, items| {
            let selector: &mut SelectorView = compositor.get_mut_surface_by_name("selector");
            if selector.opened_by() != "finder" {
                return;
            }

            let selector_items = items
                .into_iter()
                .map(|item| {
                    let content = match &item {
                        FinderItem::File(path) => SelectorContent::Normal {
                            label: path.to_owned(),
                            sub_label: Some("(file)".to_owned()),
                        },
                        FinderItem::Buffer { name, .. } => SelectorContent::Normal {
                            label: name.to_owned(),
                            sub_label: Some("(buffer)".to_owned()),
                        },
                        FinderItem::SearchMatch {
                            path,
                            pos,
                            line_text,
                            before,
                            matched,
                            after,
                        } => SelectorContent::SearchMatch {
                            path: path.to_owned(),
                            pos: pos.to_owned(),
                            line_text: line_text.to_owned(),
                            before: before.to_owned(),
                            matched: matched.to_owned(),
                            after: after.to_owned(),
                        },
                        FinderItem::Action { name } => SelectorContent::Normal {
                            label: name.to_owned(),
                            sub_label: Some("(action)".to_owned()),
                        },
                    };

                    SelectorItem {
                        content,
                        selected: Box::new(move |compositor, editor| {
                            select_item(compositor, editor, item);
                        }),
                    }
                })
                .collect();

            selector.set_items(selector_items);
        },
    );
}

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

fn search_globally(
    workspace_dir: &Path,
    query: &str,
    tx: UnboundedSender<(i64, FinderItem)>,
) -> Result<()> {
    use grep::{matcher::Matcher, regex::RegexMatcherBuilder, searcher::Searcher};
    use ignore::{WalkBuilder, WalkState};

    const MAX_SEARCH_MATCHES: usize = 32;

    let query = regex::escape(query);

    let matcher = match RegexMatcherBuilder::new().case_smart(true).build(&query) {
        Ok(matcher) => matcher,
        Err(err) => {
            bail!("invalid regex: {}", err);
        }
    };

    let counter = Arc::new(AtomicUsize::new(0));
    WalkBuilder::new(workspace_dir).build_parallel().run(|| {
        let matcher = matcher.clone();
        let tx = tx.clone();
        let counter = counter.clone();
        Box::new(move |dirent| {
            if let Ok(dirent) = dirent {
                let meta = dirent.metadata().unwrap();
                if !meta.is_file() {
                    return WalkState::Continue;
                }

                trace!("grep: {}", dirent.path().display());
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
                                    let before_text = &line[..m.start()];
                                    let matched_text = &line[m.start()..m.end()];
                                    let after_text = &line[m.end()..];
                                    trace!(
                                        "{}:{}: {}\x1b[1;31m{}\x1b[0m{}",
                                        dirent.path().display(),
                                        lineno,
                                        before_text,
                                        matched_text,
                                        after_text
                                    );

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
                                    let item = FinderItem::SearchMatch {
                                        path: path_str,
                                        pos: Position::new((lineno as usize) - 1, x),
                                        line_text,
                                        before: ..m_start,
                                        matched: m_start..m_end,
                                        after: m_end..,
                                    };

                                    let _ = tx.send((0, item));

                                    // Abort the search if we have seen enough matches.
                                    counter.fetch_add(1, Ordering::SeqCst) < MAX_SEARCH_MATCHES
                                })
                                .oops();

                            // Abort the search if we have seen enough matches.
                            Ok(counter.load(Ordering::SeqCst) < MAX_SEARCH_MATCHES)
                        }),
                    )
                    .oops();
            }

            WalkState::Continue
        })
    });

    Ok(())
}

fn search_paths(
    workspace_dir: &Path,
    query: &str,
    tx: UnboundedSender<(i64, FinderItem)>,
    visited_paths: &HashSet<PathBuf>,
) {
    use ignore::{WalkBuilder, WalkState};

    WalkBuilder::new(workspace_dir).build_parallel().run(|| {
        let matcher = build_fuzzy_matcher();
        let tx = tx.clone();
        Box::new(move |dirent| {
            if let Ok(dirent) = dirent {
                let meta = dirent.metadata().unwrap();
                if !meta.is_file() {
                    return WalkState::Continue;
                }

                if visited_paths.contains(dirent.path()) {
                    return WalkState::Continue;
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

                        let _ = tx.send((score, FinderItem::File(path.to_owned())));
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
