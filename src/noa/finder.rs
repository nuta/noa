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
    fuzzy_set::FuzzySet,
    line_edit::LineEdit,
    selector::Selector,
    ui::{Canvas, Compositor, Context, Event, HandledEvent, Layout, RectSize, Surface},
};

enum Item {
    File(PathBuf),
}

pub struct Finder {
    query: LineEdit,
    selector: Arc<Mutex<Selector<Item>>>,
}

impl Finder {
    pub fn new(ctx: &mut Context) -> Finder {
        let selector = Arc::new(Mutex::new(Selector::new()));

        tokio::spawn(update_items(
            ctx.editor.workspace_dir().to_owned(),
            ctx.event_tx.clone(),
            selector.clone(),
            "".to_owned(),
        ));

        Finder {
            query: LineEdit::new(),
            selector,
        }
    }
}

impl Surface for Finder {
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

        let selector = self.selector.lock();
        for (i, (active, item)) in selector.items().enumerate() {
            let title = match item {
                Item::File(path) => path.to_str().unwrap(),
            };

            canvas.draw_str(2 + i, 1, &title);
        }

        canvas.draw_str(1, 1, &self.query.text());
        canvas.draw_borders(0, 0, canvas.height() - 1, canvas.width() - 1);
    }

    fn handle_key_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        key: KeyEvent,
    ) -> HandledEvent {
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
                return HandledEvent::Consumed;
            }
            (NONE, KeyCode::Backspace) => {
                self.query.backspace();
                true
            }
            (NONE, KeyCode::Enter) => {
                compositor.pop_layer();
                // TODO:
                return HandledEvent::Consumed;
            }
            _ => {
                trace!("finder: unhandled key event: {:?}", key);
                return HandledEvent::Consumed;
            }
        };

        tokio::spawn(update_items(
            ctx.editor.workspace_dir().to_owned(),
            ctx.event_tx.clone(),
            self.selector.clone(),
            self.query.text(),
        ));

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        ctx: &mut Context,
        _compositor: &mut Compositor,
        input: &str,
    ) -> HandledEvent {
        self.query.insert(input);
        HandledEvent::Consumed
    }
}

async fn update_items(
    workspace_dir: PathBuf,
    event_tx: UnboundedSender<Event>,
    selector: Arc<Mutex<Selector<Item>>>,
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
    let mut selector = selector.lock();
    selector.clear();
    for results in iter {
        for item in results.into_inner().into_iter() {
            selector.push(item.value);
        }
    }

    event_tx.send(Event::ReDraw);
}
