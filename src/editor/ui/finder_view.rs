use std::{
    cmp::{max, min},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, bail, Result};
use arc_swap::ArcSwap;
use futures::{executor::block_on, stream::FuturesUnordered, StreamExt};
use grep::searcher::SinkError;
use noa_buffer::{
    cursor::{Cursor, Position, Range},
    display_width::DisplayWidth,
};
use noa_common::{
    collections::{fuzzy_set::FuzzySet, prioritized_vec::PrioritizedVec},
    oops::OopsExt,
};
use noa_compositor::{
    canvas::{CanvasViewMut, Color, Decoration, Style},
    line_edit::LineEdit,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyEvent, KeyModifiers},
    Compositor,
};
use parking_lot::RwLock;
use tokio::sync::{mpsc::UnboundedSender, Notify};

use crate::{
    document::DocumentId,
    editor::Editor,
    theme::{theme_for, ThemeKey},
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

type IntoFinderItem = fn(String) -> FinderItem;

pub struct FinderView {
    render_request: Arc<Notify>,
    workspace_dir: PathBuf,
    active: bool,
    items: Arc<ArcSwap<Vec<FinderItem>>>,
    item_selected: usize,
    num_items_shown: usize,
    input: LineEdit,
}

impl FinderView {
    pub fn new(editor: &Editor, render_request: Arc<Notify>, workspace_dir: &Path) -> FinderView {
        let mut finder = FinderView {
            render_request,
            workspace_dir: workspace_dir.to_path_buf(),
            active: false,
            items: Arc::new(ArcSwap::from_pointee(Vec::new())),
            item_selected: 0,
            num_items_shown: 0,
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

    pub fn update(&mut self, editor: &Editor) {
        let mut providers = FuturesUnordered::new();
        let workspace_dir = self.workspace_dir.clone();
        let render_request = self.render_request.clone();
        let items = self.items.clone();
        let query = self.input.text();

        // Buffers.
        let mut buffers = FuzzySet::new();
        for (id, doc) in editor.documents.documents().iter() {
            buffers.insert(
                doc.name().to_owned(),
                FinderItem::Buffer {
                    name: doc.name().to_owned(),
                    path: doc
                        .path()
                        .map(|p| p.to_str().unwrap())
                        .unwrap_or_else(|| doc.name())
                        .to_owned(),
                    id: *id,
                },
                100,
            );
        }

        tokio::spawn(async move {
            let mut all_items = PrioritizedVec::new(32);
            match query.chars().next() {
                Some('/') => {
                    if query.len() < 4 {
                        warn!("too short query");
                        return;
                    }

                    match search_globally(&workspace_dir, &query[1..]).await {
                        Ok(new_items) => {
                            items.store(Arc::new(new_items));
                        }
                        Err(err) => {
                            // TODO:
                            warn!("failed to search globally: {}", err);
                        }
                    }
                    render_request.notify_one();
                    return;
                }
                _ => {
                    providers.push(tokio::spawn(async move { buffers }));
                    providers.push(tokio::task::spawn_blocking(|| scan_paths(workspace_dir)));

                    // Files in the workspace directory.
                }
            }

            while let Some(results) = providers.next().await {
                let results = match results {
                    Ok(results) => results,
                    Err(err) => {
                        warn!("failed to provide finder items: {}", err);
                        continue;
                    }
                };

                let mut visited_paths = Vec::new();
                for (s, item, score) in results.query(&query) {
                    // Ignore already opened files.
                    match item {
                        FinderItem::File(path) | FinderItem::Buffer { path, .. } => {
                            if visited_paths.contains(path) {
                                continue;
                            }

                            visited_paths.push(path.clone());
                        }
                        _ => {}
                    }

                    all_items.insert(score, item.to_owned());
                }

                items.store(Arc::new(all_items.sorted_vec()));
                render_request.notify_one();
            }
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

    fn layout(&self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
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
        Some((1, 2 + self.input.cursor_position()))
    }

    fn render(&mut self, _editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        self.input.relocate_scroll(canvas.width());

        self.num_items_shown = canvas.height() - 2;

        canvas.write_str(
            0,
            1,
            truncate_to_width(&self.input.text(), canvas.width() - 2),
        );
        canvas.apply_style(0, 1, canvas.width() - 2, theme_for(ThemeKey::FinderInput));

        for (i, item) in self
            .items
            .load()
            .iter()
            .take(self.num_items_shown)
            .enumerate()
        {
            if i == self.item_selected {
                canvas.apply_style(
                    2 + i,
                    1,
                    canvas.width() - 8,
                    theme_for(ThemeKey::FinderSelectedItem),
                );
            }

            match item {
                FinderItem::File(path) => {
                    canvas.write_str(2 + i, 2, truncate_to_width(path, canvas.width() - 2));
                    canvas.write_char_with_style(
                        2 + i,
                        1,
                        'F',
                        Style {
                            bg: Color::Black,
                            ..Default::default()
                        },
                    );
                }
                FinderItem::Buffer { name, .. } => {
                    canvas.write_str(2 + i, 2, truncate_to_width(name, canvas.width() - 2));
                    canvas.write_char_with_style(
                        2 + i,
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
                    canvas.write_str(2 + i, 2, truncate_to_width(&s, canvas.width() - 2));

                    let x = 2 + before_text.display_width();
                    canvas.apply_style(
                        2 + i,
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
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        match (key.code, key.modifiers) {
            (KeyCode::Enter, NONE) => {
                match self.items.load().get(self.item_selected) {
                    Some(item) => {
                        info!("finder: selected item: {:?}", item);
                        match item {
                            FinderItem::File(path) => {
                                if let Err(err) = editor.documents.open_file(Path::new(path)) {
                                    notify_anyhow_error!(err);
                                }
                            }
                            FinderItem::Buffer { id, .. } => {
                                editor.documents.switch_current(*id);
                            }
                            FinderItem::SearchMatch { path, pos, .. } => {
                                match editor.documents.open_file(Path::new(path)) {
                                    Ok(doc) => {
                                        doc.buffer_mut().move_main_cursor_to(*pos);
                                        doc.flashes_mut().flash(Range::from_positions(*pos, *pos));
                                    }
                                    Err(err) => {
                                        notify_anyhow_error!(err);
                                    }
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
                    min(self.num_items_shown, self.items.load().len()).saturating_sub(1),
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

fn scan_paths(workspace_dir: PathBuf) -> FuzzySet<FinderItem> {
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
                                extra += (100 / max(1, min(elapsed.as_secs(), 360))) as isize;
                                extra += (100 / max(1, elapsed.as_secs())) as isize;
                            }
                        }

                        // Recently modified.
                        if let Ok(mtime) = meta.modified() {
                            if let Ok(elapsed) = mtime.elapsed() {
                                extra += (10
                                    / max(
                                        1,
                                        min(elapsed.as_secs() / (3600 * 24 * 30), 3600 * 24 * 30),
                                    )) as isize;
                                extra += (100 / max(1, min(elapsed.as_secs(), 360))) as isize;
                                extra += (100 / max(1, elapsed.as_secs())) as isize;
                            }
                        }

                        let path = path.strip_prefix("./").unwrap_or(path);
                        paths
                            .write()
                            .insert(path, FinderItem::File(path.to_owned()), extra);
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

async fn search_globally(workspace_dir: &Path, raw_query: &str) -> Result<Vec<FinderItem>> {
    use grep::{matcher::Matcher, regex::RegexMatcherBuilder, searcher::Searcher};
    use ignore::{WalkBuilder, WalkState};

    let query = regex::escape(raw_query);

    let matcher = match RegexMatcherBuilder::new().case_smart(true).build(&query) {
        Ok(matcher) => matcher,
        Err(err) => {
            bail!("invalid regex: {}", err);
        }
    };

    let mut items = Arc::new(RwLock::new(PrioritizedVec::new(32)));
    WalkBuilder::new(workspace_dir).build_parallel().run(|| {
        Box::new(|dirent| {
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
                        Utf8Sink(|lineno, range, line| {
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

                                    let m_start = min(m.start(), line_text.len());
                                    let m_end = min(m.end(), line_text.len());
                                    let mut items = items.write();
                                    items.insert(
                                        0,
                                        FinderItem::SearchMatch {
                                            path: dirent.path().to_str().unwrap().to_owned(),
                                            pos: Position::new((lineno as usize) - 1, x),
                                            line_text,
                                            before: ..m_start,
                                            matched: m_start..m_end,
                                            after: m_end..,
                                        },
                                    );

                                    /* continue finding */
                                    items.len() < 32
                                })
                                .unwrap();

                            /* continue searching */
                            Ok(true)
                        }),
                    )
                    .oops();
            }

            WalkState::Continue
        })
    });

    let results = items.read().sorted_vec();
    Ok(results)
}
