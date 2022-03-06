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
use arc_swap::ArcSwap;
use crossbeam::queue::SegQueue;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use grep::searcher::SinkError;
use noa_buffer::{cursor::Position, display_width::DisplayWidth};
use noa_common::{oops::OopsExt, prioritized_vec::PrioritizedVec};
use noa_compositor::{
    canvas::{CanvasViewMut, Color, Decoration, Style},
    line_edit::LineEdit,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyEvent, KeyModifiers},
    Compositor,
};
use parking_lot::Mutex;
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedSender},
    Notify,
};

use crate::{
    completion::build_fuzzy_matcher, document::DocumentId, editor::Editor, theme::theme_for,
};

use super::helpers::truncate_to_width;

#[derive(Clone, Debug)]
enum FinderItem {
    File(String),
    Buffer {
        name: String,
        path: String,
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
}

pub struct FinderView {
    render_request: Arc<Notify>,
    workspace_dir: PathBuf,
    active: bool,
    items: Vec<FinderItem>,
    item_selected: usize,
    num_visible_items: usize,
    input: LineEdit,
}

impl FinderView {
    pub fn new(editor: &Editor, render_request: Arc<Notify>, workspace_dir: &Path) -> FinderView {
        let mut finder = FinderView {
            render_request,
            workspace_dir: workspace_dir.to_path_buf(),
            active: false,
            items: Vec::new(),
            item_selected: 0,
            num_visible_items: 0,
            input: LineEdit::new(),
        };

        finder.update(editor);
        finder
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if active {
            self.item_selected = 0;
            self.input.clear();
        }
    }

    fn set_items(&mut self, items: Vec<FinderItem>) {
        self.items = items;
        self.item_selected = 0;
        self.num_visible_items = min(self.items.len(), 20);
    }

    pub fn update(&mut self, editor: &Editor) {
        let workspace_dir = self.workspace_dir.clone();
        let query = self.input.text();

        let mut new_items = PrioritizedVec::new();
        let mut visited_paths = HashSet::new();

        // Buffers.
        let matcher = build_fuzzy_matcher();
        for (id, doc) in editor.documents.documents().iter() {
            if let Some(score) = matcher.fuzzy_match(doc.path_in_str(), &query) {
                new_items.insert(
                    score + 100,
                    FinderItem::Buffer {
                        name: doc.name().to_owned(),
                        path: doc.path().to_str().unwrap().to_owned(),
                        id: *id,
                    },
                );
            }

            visited_paths.insert(doc.path().to_owned());
        }

        let (items_tx, mut items_rx) = unbounded_channel();
        tokio::task::spawn_blocking(move || match query.chars().next() {
            Some('/') => {
                if query.len() < 4 {
                    warn!("too short query");
                    return;
                }

                if let Err(err) = search_globally(&workspace_dir, &query[1..], items_tx) {
                    notify_warn!("failed to search globally: {}", err);
                    
                }
            }
            _ => {
                scan_paths(&workspace_dir, &query, items_tx, &visited_paths);
            }
        });

        // let callback = editor.register_callback(|compositor, _editor| {
        //     // let finder_view: &mut FinderView = compositor.get_mut_surface_by_name("finder");
        //     // TODO:
        //     // finder_view.set_items(new_items);
        // });

        let callback_request = self.render_request.clone();
        tokio::spawn(async move {
            while let Some((score, item)) = items_rx.recv().await {
                new_items.insert(score, item);
            }

            // TODO:
            // callback_request.send(callback);
        });
    }
}

impl Surface for FinderView {
    type Context = Editor;

    fn name(&self) -> &str {
        "finder"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _editor: &mut Editor) -> bool {
        self.active
    }

    fn layout(&mut self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        let height = min(24, screen_size.height);
        (
            Layout::Fixed {
                y: screen_size.height.saturating_sub(height),
                x: 0,
            },
            RectSize {
                height,
                width: screen_size.width,
            },
        )
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        if self.active {
            Some((0, 1 + self.input.cursor_position()))
        } else {
            None
        }
    }

    fn render(&mut self, _editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        self.input.relocate_scroll(canvas.width());

        self.num_visible_items = canvas.height() - 2;

        canvas.write_str(
            0,
            1,
            truncate_to_width(&self.input.text(), canvas.width() - 2),
        );
        canvas.apply_style(0, 1, canvas.width() - 2, theme_for("finder.input"));

        for (i, item) in self.items.iter().take(self.num_visible_items).enumerate() {
            if i == self.item_selected {
                canvas.apply_style(1 + i, 1, canvas.width() - 8, theme_for("finder.selected"));
            }

            match item {
                FinderItem::File(path) => {
                    canvas.write_str(1 + i, 2, truncate_to_width(path, canvas.width() - 2));
                    canvas.write_char_with_style(
                        1 + i,
                        1,
                        'F',
                        Style {
                            bg: Color::Black,
                            ..Default::default()
                        },
                    );
                }
                FinderItem::Buffer { name, .. } => {
                    canvas.write_str(1 + i, 2, truncate_to_width(name, canvas.width() - 2));
                    canvas.write_char_with_style(
                        1 + i,
                        1,
                        'B',
                        Style {
                            bg: Color::Cyan,
                            ..Default::default()
                        },
                    );
                }
                FinderItem::SearchMatch {
                    path,
                    pos,
                    line_text,
                    before,
                    after,
                    matched,
                } => {
                    let before_text = &line_text[..before.end];
                    let matched_text = &line_text[matched.start..matched.end];
                    let after_text = &line_text[after.start..];
                    let s = format!(
                        "{before_text}{matched_text}{after_text} ({path}:{lineno})",
                        lineno = pos.y + 1
                    );
                    canvas.write_str(1 + i, 2, truncate_to_width(&s, canvas.width() - 2));

                    let x = 2 + before_text.display_width();
                    canvas.apply_style(
                        1 + i,
                        x,
                        min(canvas.width(), x + matched_text.display_width()),
                        Style {
                            fg: Color::Red,
                            bg: Color::Reset,
                            deco: Decoration::underline(),
                        },
                    );
                }
            }
        }
    }

    fn handle_key_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        editor: &mut Editor,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        match (key.code, key.modifiers) {
            (KeyCode::Enter, NONE) => {
                match self.items.get(self.item_selected) {
                    Some(item) => {
                        info!("finder: selected item: {:?}", item);
                        match item {
                            FinderItem::Buffer { id, .. } => {
                                editor.documents.switch_by_id(*id);
                            }
                            FinderItem::File(path) => {
                                let path = Path::new(path);
                                match editor.documents.switch_by_path(path) {
                                    Some(_) => {}
                                    None => match editor.open_file(path, None) {
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
                                let path = Path::new(path);
                                match editor.documents.switch_by_path(path) {
                                    Some(_) => {}
                                    None => match editor.open_file(path, Some(*pos)) {
                                        Ok(id) => {
                                            editor.documents.switch_by_id(id);
                                        }
                                        Err(err) => {
                                            notify_anyhow_error!(err);
                                        }
                                    },
                                }
                            }
                        }

                        self.active = false;
                        return HandledEvent::Consumed;
                    }
                    None => {
                        notify_error!("items changed (try again!)");
                    }
                };
            }
            (KeyCode::Down, NONE) => {
                self.item_selected = min(
                    self.item_selected + 1,
                    min(self.num_visible_items, self.items.len()).saturating_sub(1),
                );
            }
            (KeyCode::Up, NONE) => {
                self.item_selected = self.item_selected.saturating_sub(1);
            }
            (KeyCode::Char('q'), CTRL) => {
                self.active = false;
            }
            _ => {
                self.input.consume_key_event(key);
                self.update(editor);
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor<Editor>,
        editor: &mut Editor,
        input: &str,
    ) -> HandledEvent {
        self.input.insert(&input.replace('\n', " "));
        self.update(editor);
        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        _ctx: &mut Self::Context,
        _kind: noa_compositor::terminal::MouseEventKind,
        _modifiers: noa_compositor::terminal::KeyModifiers,
        _surface_y: usize,
        _surface_x: usize,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }
}

fn scan_paths(
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

                        tx.send((score, FinderItem::File(path.to_owned())));
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

    let mut counter = Arc::new(AtomicUsize::new(0));
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

                                    tx.send((0, item));

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
