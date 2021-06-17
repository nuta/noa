use std::{
    cmp::{max, min, Ordering},
    collections::{binary_heap, BinaryHeap},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    style::{Attribute, Color},
};
use noa_buffer::Snapshot;
use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    fuzzy_set::FuzzySet,
    line_edit::LineEdit,
    selector::Selector,
    ui::{
        truncate_to_width, Canvas, Compositor, Context, DisplayWidth, Event, HandledEvent, Layout,
        RectSize, Surface,
    },
};

const MIN_WIDTH: usize = 16;
const MAX_WIDTH: usize = 64;
const MAX_HEIGHT: usize = 16;

enum Item {
    Word(String),
}

pub struct Completion {
    selector: Arc<Mutex<Selector<Item>>>,
}

impl Completion {
    pub fn new(ctx: &mut Context) -> Completion {
        let selector = Arc::new(Mutex::new(Selector::new()));
        let buffer = ctx.editor.current_buffer().read();
        let current_word = buffer.current_word().unwrap_or_else(|| "".to_owned());
        let snapshot = buffer.take_snapshot();

        tokio::spawn(update_completion(
            ctx.event_tx.clone(),
            selector,
            current_word,
            snapshot,
        ));

        Completion {
            selector: Arc::new(Mutex::new(Selector::new())),
        }
    }
}

impl Surface for Completion {
    fn name(&self) -> &str {
        "popup"
    }

    fn is_visible(&self) -> bool {
        !self.selector.lock().is_empty()
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        let selector = self.selector.lock();

        // Determine the maximum item width.
        let max_width = selector
            .items()
            .take(16)
            .fold(MIN_WIDTH, |max_width, (_, item)| {
                let width = match item {
                    Item::Word(s) => s.len(),
                };

                max(max_width, width)
            });

        let width = max_width + 2 /* border */;
        let height = selector.len() + 2 /* border */;
        info!("relayout {}", height);
        (Layout::AroundCursor, RectSize { height, width })
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        canvas.clear();
        canvas.draw_borders(0, 0, canvas.height() - 1, canvas.width() - 1);

        info!("canvas.height()={}", canvas.height());
        for (i, (active, item)) in self
            .selector
            .lock()
            .items()
            .take(canvas.height().saturating_sub(2))
            .enumerate()
        {
            let text = match item {
                Item::Word(text) => text,
            };

            info!("{}: {}", i, text);
            let y = 1 + i;
            let x = 1;
            canvas.draw_str(y, x, truncate_to_width(text, canvas.width() - 1));

            if active {
                let attrs = [Attribute::Underlined, Attribute::Bold];
                canvas.set_attrs(y, x, y + 1, canvas.width() - 1, (&attrs[..]).into());
            }
        }
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

        if matches!(key.code, KeyCode::Char(_)) {
            let buffer = ctx.editor.current_buffer().read();
            let current_word = buffer.current_word().unwrap_or_else(|| "".to_owned());
            let snapshot = buffer.take_snapshot();

            tokio::spawn(update_completion(
                ctx.event_tx.clone(),
                self.selector.clone(),
                current_word,
                snapshot,
            ));
        }

        let mut selector = self.selector.lock();
        if selector.is_empty() {
            return HandledEvent::Ignored;
        }

        match (key.modifiers, key.code) {
            (NONE, KeyCode::Esc) => {
                selector.clear();
                HandledEvent::Consumed
            }
            (NONE, KeyCode::Up) => {
                selector.select_prev();
                HandledEvent::Consumed
            }
            (NONE, KeyCode::Down) => {
                selector.select_next();
                HandledEvent::Consumed
            }
            (NONE, KeyCode::Enter) => {
                if let Some(selected) = selector.selected() {
                    match selected {
                        Item::Word(word) => {
                            let mut buffer = ctx.editor.current_buffer().write();
                            if let Some(range) = buffer.current_word_range() {
                                buffer.select_by_ranges(&[range]);
                                buffer.insert(word);
                            }
                        }
                    }
                }

                selector.clear();
                HandledEvent::Consumed
            }
            _ => HandledEvent::Ignored,
        }
    }

    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut Context,
        _compositor: &mut Compositor,
        _input: &str,
    ) -> HandledEvent {
        // TODO:
        HandledEvent::Consumed
    }
}

async fn update_completion(
    event_tx: UnboundedSender<Event>,
    selector: Arc<Mutex<Selector<Item>>>,
    query: String,
    snapshot: Arc<Snapshot>,
) {
    use ignore::{WalkBuilder, WalkState};

    // Word completion.
    let word_comp = async move {
        let mut results = FuzzySet::with_capacity(32);
        for word in snapshot.words() {
            if word == query {
                continue;
            }

            if let Some(m) = sublime_fuzzy::best_match(&query, word) {
                results.push(m.score(), Item::Word(word.to_owned()));
            }
        }

        results
    };

    // Merge results.
    let iter = futures::future::join_all(vec![word_comp]).await;
    let mut selector = selector.lock();
    selector.clear();
    for results in iter {
        for item in results.into_iter() {
            selector.push(item.value);
        }
    }

    info!("update completion to {}", selector.len());
    event_tx.send(Event::ReDraw);
}
