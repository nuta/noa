use std::{
    cmp::{max, min},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::anyhow;
use futures::{executor::block_on, stream::FuturesUnordered, StreamExt};
use noa_common::prioritized_vec::PrioritizedVec;
use noa_compositor::{
    canvas::CanvasViewMut,
    line_edit::LineEdit,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyEvent, KeyModifiers},
    Compositor,
};
use parking_lot::RwLock;
use tokio::sync::{mpsc::UnboundedSender, Notify};

use crate::{editor::Editor, fuzzy::FuzzySet};

use super::helpers::truncate_to_width;

#[derive(Clone, Debug)]
enum FinderItem {
    Path(String),
}

type IntoFinderItem = fn(String) -> FinderItem;

pub struct FinderView {
    render_request: Arc<Notify>,
    workspace_dir: PathBuf,
    active: bool,
    items: Arc<RwLock<Vec<FinderItem>>>,
    item_selected: usize,
    input: LineEdit,
}

impl FinderView {
    pub fn new(render_request: Arc<Notify>, workspace_dir: &Path) -> FinderView {
        let mut finder = FinderView {
            render_request,
            workspace_dir: workspace_dir.to_path_buf(),
            active: false,
            items: Arc::new(RwLock::new(Vec::new())),
            item_selected: 0,
            input: LineEdit::new(),
        };

        finder.update();
        finder
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn update(&mut self) {
        let mut providers = FuturesUnordered::new();
        let workspace_dir = self.workspace_dir.clone();
        let render_request = self.render_request.clone();
        let items = self.items.clone();
        let query = self.input.text();

        tokio::spawn(async move {
            let mut all_items = PrioritizedVec::new(32);
            providers.push(scan_paths(workspace_dir));
            while let Some((results, into_item)) = providers.next().await {
                for (s, score) in results.query(&query) {
                    all_items.insert(score, into_item(s.to_string()));
                }

                *items.write() = all_items.sorted_vec();
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
        (
            Layout::Center,
            RectSize {
                height: min(32, screen_size.height.saturating_sub(5)),
                width: min(80, screen_size.width),
            },
        )
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        Some((1, 2 + self.input.cursor_position()))
    }

    fn render(&mut self, _editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        self.input.relocate_scroll(canvas.width() - 4);

        let max_num_items = min(canvas.height() / 2, 16);

        canvas.draw_borders(0, 0, canvas.height() - 1, canvas.width() - 1);
        canvas.write_str(
            1,
            2,
            truncate_to_width(&self.input.text(), canvas.width() - 4),
        );

        for (i, item) in self.items.read().iter().take(max_num_items).enumerate() {
            match item {
                FinderItem::Path(path) => {
                    canvas.write_str(2 + i, 1, truncate_to_width(&path, canvas.width() - 2));
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
                match self.items.read().get(self.item_selected) {
                    Some(item) => {
                        info!("finder: selected item: {:?}", item);
                    }
                    None => {
                        editor
                            .notifications
                            .error(anyhow!("items changed (try again!)"));
                    }
                };
            }
            (KeyCode::Down, NONE) => {
                self.item_selected = min(self.item_selected - 1, self.items.read().len());
            }
            (KeyCode::Up, NONE) => {
                self.item_selected = self.item_selected.saturating_sub(1);
            }
            _ => {
                let result = self.input.consume_key_event(key);
                self.update();
                return result;
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor<Editor>,
        _editor: &mut Editor,
        input: &str,
    ) -> HandledEvent {
        self.input.insert(&input.replace('\n', " "));
        self.update();
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
        trace!("_kind={:?}", _kind);
        HandledEvent::Consumed
    }
}

async fn scan_paths(workspace_dir: PathBuf) -> (FuzzySet, impl Fn(String) -> FinderItem) {
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

    (paths.into_inner(), FinderItem::Path)
}
