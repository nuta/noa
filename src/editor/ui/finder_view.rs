use std::{
    cmp::{max, min},
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::{executor::block_on, stream::FuturesUnordered, StreamExt};
use noa_common::prioritized_vec::PrioritizedVec;
use noa_compositor::{
    canvas::CanvasViewMut,
    line_edit::LineEdit,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::KeyEvent,
    Compositor,
};
use parking_lot::RwLock;
use tokio::sync::{mpsc::UnboundedSender, Notify};

use crate::{editor::Editor, fuzzy::FuzzySet};

use super::helpers::truncate_to_width;

enum FinderItem {
    Path(String),
}

type IntoFinderItem = fn(String) -> FinderItem;

pub struct FinderView {
    render_request: Arc<Notify>,
    workspace_dir: PathBuf,
    active: bool,
    items: Arc<RwLock<PrioritizedVec<FinderItem>>>,
    input: LineEdit,
}

impl FinderView {
    pub fn new(render_request: Arc<Notify>, workspace_dir: &Path) -> FinderView {
        FinderView {
            render_request,
            workspace_dir: workspace_dir.to_path_buf(),
            active: false,
            items: Arc::new(RwLock::new(PrioritizedVec::new(32))),
            input: LineEdit::new(),
        }
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
            providers.push(scan_paths(workspace_dir));
            while let Some((results, into_item)) = providers.next().await {
                for (s, score) in results.query(&query) {
                    items.write().insert(score, into_item(s.to_string()));
                }
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

        canvas.draw_borders(0, 0, canvas.height() - 1, canvas.width() - 1);
        canvas.write_str(
            1,
            2,
            truncate_to_width(&self.input.text(), canvas.width() - 4),
        );
    }

    fn handle_key_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        _editor: &mut Editor,
        key: KeyEvent,
    ) -> HandledEvent {
        let result = self.input.consume_key_event(key);
        self.update();
        result
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor<Editor>,
        _editor: &mut Editor,
        input: &str,
    ) -> HandledEvent {
        self.input.insert(input);
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

    (paths.into_inner(), |path| FinderItem::Path(path))
}
