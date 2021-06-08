use std::{
    cmp::{max, min, Ordering},
    collections::BinaryHeap,
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;

use crate::ui::{Canvas, Compositor, Context, Event, Layout, RectSize, Surface};

enum Item {
    File(String),
}

pub struct FinderSurface {
    query: String,
    items: Arc<Mutex<FuzzySet>>,
}

impl FinderSurface {
    pub fn new(ctx: &mut Context) -> FinderSurface {
        let items = Arc::new(Mutex::new(FuzzySet::with_capacity(64)));
        tokio::spawn(update_items(
            ctx.editor.workspace_dir().to_owned(),
            ctx.event_tx.clone(),
            items.clone(),
            "".to_owned(),
        ));
        FinderSurface {
            query: String::new(),
            items,
        }
    }
}

impl Surface for FinderSurface {
    fn name(&self) -> &str {
        "finder"
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        let rect_size = RectSize {
            width: min(max(screen_size.width, 32), 80),
            height: min(max(screen_size.height, 8), 16),
        };
        (Layout::Center, rect_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        // TODO:
        None
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        self.render_all(ctx, canvas)
    }

    fn render_all(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        // TODO:
    }

    fn handle_key_event(&mut self, ctx: &mut Context, compositor: &mut Compositor, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let updated = match (key.modifiers, key.code) {
            (NONE, KeyCode::Enter) => false,
            _ => {
                return;
            }
        };

        tokio::spawn(update_items(
            ctx.editor.workspace_dir().to_owned(),
            ctx.event_tx.clone(),
            self.items.clone(),
            self.query.clone(),
        ));
    }

    fn handle_key_batch_event(
        &mut self,
        ctx: &mut Context,
        _compositor: &mut Compositor,
        input: &str,
    ) {
        // TODO:
    }
}

struct FuzzyItem {
    score: isize,
    text: String,
}

impl PartialEq for FuzzyItem {
    fn eq(&self, other: &FuzzyItem) -> bool {
        self.score.eq(&other.score)
    }
}

impl Eq for FuzzyItem {}

impl PartialOrd for FuzzyItem {
    fn partial_cmp(&self, other: &FuzzyItem) -> Option<Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl Ord for FuzzyItem {
    fn cmp(&self, other: &FuzzyItem) -> Ordering {
        self.score.cmp(&other.score)
    }
}

struct FuzzySet {
    capacity: usize,
    items: BinaryHeap<FuzzyItem>,
}

impl FuzzySet {
    pub fn with_capacity(capacity: usize) -> FuzzySet {
        FuzzySet {
            capacity,
            items: BinaryHeap::with_capacity(capacity + 1),
        }
    }

    pub fn push(&mut self, score: isize, text: String) {
        self.items.push(FuzzyItem { score, text });
        self.items.pop();
    }
}

async fn update_items(
    workspace_dir: PathBuf,
    event_tx: UnboundedSender<Event>,
    items: Arc<Mutex<FuzzySet>>,
    query: String,
) {
    use ignore::{WalkBuilder, WalkState};

    let path_resolver = tokio::spawn(async move {
        WalkBuilder::new(workspace_dir).build_parallel().run(|| {
            Box::new(|dirent| {
                if let Ok(dirent) = dirent {
                    let path = dirent.path().to_str().unwrap();
                    if let Some(m) = sublime_fuzzy::best_match(&query, path) {
                        items.lock().push(
                            m.score(),
                            sublime_fuzzy::format_simple(&m, path, "\x1b[1m\x1b[4m", "\x1b[0m"),
                        );
                    }
                }
                WalkState::Continue
            })
        });
    });

    tokio::join!(path_resolver);
}
