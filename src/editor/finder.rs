use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use fuzzy_matcher::FuzzyMatcher;

use noa_common::prioritized_vec::PrioritizedVec;
use noa_compositor::Compositor;

use once_cell::sync::Lazy;
use regex::Regex;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    actions::{execute_action_or_notify, ACTIONS},
    completion::build_fuzzy_matcher,
    document::DocumentId,
    editor::Editor,
    search::{search_paths_globally, search_texts_globally, CancelFlag, SearchMatch},
    ui::selector_view::{SelectorContent, SelectorItem, SelectorView},
};

const MAX_SEARCH_MATCHES: usize = 32;

#[derive(Clone, Debug)]
enum FinderItem {
    File(String),
    Buffer { name: String, id: DocumentId },
    SearchMatch(SearchMatch),
    Action { name: String },
}

pub fn open_finder(editor: &mut Editor, compositor: &mut Compositor<Editor>) {
    let selector: &mut SelectorView = compositor.get_mut_surface_by_name("selector");
    selector.open("finder", true, Some(Box::new(update_items)));
    update_items(editor, "");
}

fn select_item(editor: &mut Editor, compositor: &mut Compositor<Editor>, item: FinderItem) {
    info!("selected item: {:?}", item);
    match item {
        FinderItem::Buffer { id, .. } => {
            editor.documents.switch_by_id(id);
        }
        FinderItem::File(path) => {
            let path = if path.starts_with('/') {
                PathBuf::from(&path)
            } else {
                editor.workspace_dir.join(&path)
            };

            match editor.documents.switch_by_path(&path) {
                Some(_) => {}
                None => match editor.open_file(&path, None) {
                    Ok(id) => {
                        editor.documents.switch_by_id(id);
                    }
                    Err(err) => {
                        notify_anyhow_error!(err);
                    }
                },
            }
        }
        FinderItem::SearchMatch(SearchMatch { path, pos, .. }) => {
            let path = if path.starts_with('/') {
                PathBuf::from(&path)
            } else {
                editor.workspace_dir.join(&path)
            };

            match editor.documents.switch_by_path(&path) {
                Some(_) => {}
                None => match editor.open_file(&path, Some(pos)) {
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

    // Kill the currently running search workers if exist.
    let cancel_flag = CancelFlag::new();
    if let Some(prev_cancel_flag) = editor.finder_cancel_flag.replace(cancel_flag.clone()) {
        prev_cancel_flag.cancel();
    }

    // Search file contents or paths.
    let (text_items_tx, mut text_items_rx) = unbounded_channel();
    let (path_items_tx, mut path_items_rx) = unbounded_channel();
    {
        static QUERY_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\A([i]*)?(/{0,2})(.*)\z").unwrap());

        let cancel_flag = cancel_flag.clone();
        let query = query.to_owned();
        tokio::task::spawn_blocking(move || {
            let captures = QUERY_REGEX.captures(&query).unwrap();
            let flags = captures.get(1).unwrap().as_str();
            let slashes = captures.get(2).unwrap().as_str();
            let pattern = captures.get(3).unwrap().as_str();

            let case_insentive = flags.contains('i');
            match slashes {
                "//" => {
                    if let Err(err) = search_texts_globally(
                        &workspace_dir,
                        pattern,
                        text_items_tx,
                        true,
                        case_insentive,
                        cancel_flag.clone(),
                    ) {
                        notify_warn!("failed to search: {}", err);
                    }
                }
                "/" => {
                    if let Err(err) = search_texts_globally(
                        &workspace_dir,
                        pattern,
                        text_items_tx,
                        false,
                        case_insentive,
                        cancel_flag.clone(),
                    ) {
                        notify_warn!("failed to search: {}", err);
                    }
                }
                _ => {
                    if let Err(err) = search_paths_globally(
                        &workspace_dir,
                        &query,
                        path_items_tx,
                        Some(&visited_paths),
                        cancel_flag.clone(),
                    ) {
                        notify_warn!("failed to search path: {}", err);
                    }
                }
            }
        });
    }

    editor.jobs.await_in_mainloop(
        async move {
            for i in 0.. {
                if i > MAX_SEARCH_MATCHES {
                    cancel_flag.cancel();
                    break;
                }

                tokio::select! {
                    Some((_, item)) = text_items_rx.recv() => {
                        items.insert(0, FinderItem::SearchMatch(item));
                    }
                    Some((_, path)) = path_items_rx.recv() => {
                        items.insert(0, FinderItem::File(path));
                    }
                    else => {
                        break;
                    }
                }
            }

            items.into_sorted_vec()
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
                        FinderItem::SearchMatch(SearchMatch {
                            path,
                            pos,
                            line_text,
                            byte_range,
                        }) => SelectorContent::SearchMatch {
                            path: path.to_owned(),
                            pos: pos.to_owned(),
                            line_text: line_text.to_owned(),
                            byte_range: byte_range.to_owned(),
                        },
                        FinderItem::Action { name } => SelectorContent::Normal {
                            label: name.to_owned(),
                            sub_label: Some("(action)".to_owned()),
                        },
                    };

                    SelectorItem {
                        content,
                        selected: Box::new(move |editor, compositor| {
                            select_item(editor, compositor, item);
                        }),
                    }
                })
                .collect();

            selector.set_items(selector_items);
        },
    );
}
