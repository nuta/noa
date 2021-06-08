use std::{
    cmp::{max, min, Ordering},
    collections::{binary_heap, BinaryHeap},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    style::Color,
};
use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    line_edit::LineEdit,
    ui::{Canvas, Compositor, Context, Event, Layout, RectSize, Surface},
};

enum Item {
    File(PathBuf),
}

pub struct FinderSurface {
    query: LineEdit,
    selected_item_index: usize,
    items: Arc<Mutex<Vec<Item>>>,
}

impl FinderSurface {
    pub fn new(ctx: &mut Context) -> FinderSurface {
        let items = Arc::new(Mutex::new(Vec::with_capacity(128)));

        tokio::spawn(update_items(
            ctx.editor.workspace_dir().to_owned(),
            ctx.event_tx.clone(),
            items.clone(),
            "".to_owned(),
        ));

        FinderSurface {
            query: LineEdit::new(),
            selected_item_index: 0,
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
        Some((1, 1 + self.query.cursor()))
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        self.render_all(ctx, canvas)
    }

    fn render_all(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        canvas.clear();

        let items = self.items.lock();
        for (i, item) in items.iter().enumerate() {
            let title = match item {
                Item::File(path) => path.to_str().unwrap(),
            };

            canvas.draw_str(2 + i, 1, &title);
        }

        canvas.draw_str(1, 1, &self.query.text());
        canvas.draw_borders(0, 0, canvas.height() - 1, canvas.width() - 1);
    }

    fn handle_key_event(&mut self, ctx: &mut Context, compositor: &mut Compositor, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let updated = match (key.modifiers, key.code) {
            (NONE, KeyCode::Char(ch)) => {
                self.query.insert_char(ch);
                true
            }
            (NONE, KeyCode::Esc) => {
                compositor.pop_layer();
                return;
            }
            (NONE, KeyCode::Backspace) => {
                self.query.backspace();
                true
            }
            (NONE, KeyCode::Enter) => false,
            _ => {
                trace!("finder: unhandled key event: {:?}", key);
                return;
            }
        };

        tokio::spawn(update_items(
            ctx.editor.workspace_dir().to_owned(),
            ctx.event_tx.clone(),
            self.items.clone(),
            self.query.text(),
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

struct FuzzyItem<T> {
    score: isize,
    value: T,
}

impl<T> PartialEq for FuzzyItem<T> {
    fn eq(&self, other: &FuzzyItem<T>) -> bool {
        self.score.eq(&other.score)
    }
}

impl<T> Eq for FuzzyItem<T> {}

impl<T> PartialOrd for FuzzyItem<T> {
    fn partial_cmp(&self, other: &FuzzyItem<T>) -> Option<Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl<T> Ord for FuzzyItem<T> {
    fn cmp(&self, other: &FuzzyItem<T>) -> Ordering {
        self.score.cmp(&other.score)
    }
}

struct FuzzySet<T> {
    capacity: usize,
    items: BinaryHeap<FuzzyItem<T>>,
}

impl<T> FuzzySet<T> {
    pub fn with_capacity(capacity: usize) -> FuzzySet<T> {
        FuzzySet {
            capacity,
            items: BinaryHeap::with_capacity(capacity + 1),
        }
    }

    pub fn push(&mut self, score: isize, value: T) {
        self.items.push(FuzzyItem { score, value });
        if self.items.len() > self.capacity {
            self.items.pop();
        }
    }

    pub fn into_iter(self) -> binary_heap::IntoIter<FuzzyItem<T>> {
        self.items.into_iter()
    }
}

async fn update_items(
    workspace_dir: PathBuf,
    event_tx: UnboundedSender<Event>,
    items: Arc<Mutex<Vec<Item>>>,
    query: String,
) {
    use ignore::{WalkBuilder, WalkState};

    // Scan all files.
    let path_resolver = async move {
        let mut results = Mutex::new(FuzzySet::with_capacity(32));
        WalkBuilder::new(workspace_dir).build_parallel().run(|| {
            Box::new(|dirent| {
                if let Ok(dirent) = dirent {
                    let path = dirent.path().to_str().unwrap();
                    if let Some(m) = sublime_fuzzy::best_match(&query, path) {
                        results
                            .lock()
                            .push(m.score(), Item::File(dirent.path().to_owned()));
                    }
                }
                WalkState::Continue
            })
        });
        results
    };

    // Merge results.
    let iter = futures::future::join_all(vec![path_resolver]).await;
    let mut items = items.lock();
    items.clear();
    for results in iter {
        for item in results.into_inner().into_iter() {
            items.push(item.value);
        }
    }

    event_tx.send(Event::ReDraw);
}
