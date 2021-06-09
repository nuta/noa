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
    Text(String),
}

enum Kind {
    WordCompletion,
}

pub struct Completion {
    selector: Arc<Mutex<Selector<(Kind, Item)>>>,
}

impl Completion {
    pub fn new(ctx: &mut Context) -> Completion {
        let selector = Arc::new(Mutex::new(Selector::new()));

        tokio::spawn(update_completion(
            ctx.event_tx.clone(),
            selector.clone(),
            "".to_owned(),
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

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        todo!()
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        todo!()
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        self.render_all(ctx, canvas)
    }

    fn render_all(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        todo!()
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

        match (key.modifiers, key.code) {
            (NONE, KeyCode::Esc) => {
                compositor.pop_layer();
                HandledEvent::Consumed
            }
            (NONE, KeyCode::Up) => {
                self.selector.lock().select_prev();
                HandledEvent::Consumed
            }
            (NONE, KeyCode::Down) => {
                self.selector.lock().select_next();
                HandledEvent::Consumed
            }
            (NONE, KeyCode::Enter) => {
                compositor.pop_layer();
                // TODO:
                HandledEvent::Consumed
            }
            _ => {
                let current_word = ctx
                    .editor
                    .current_buffer()
                    .read()
                    .current_word()
                    .unwrap_or_else(|| "".to_owned());

                tokio::spawn(update_completion(
                    ctx.event_tx.clone(),
                    self.selector.clone(),
                    current_word,
                ));

                return HandledEvent::Ignored;
            }
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
    selector: Arc<Mutex<Selector<(Kind, Item)>>>,
    query: String,
) {
    use ignore::{WalkBuilder, WalkState};

    // Scan all files.
    let path_resolver = async move {
        let mut results = Mutex::new(FuzzySet::with_capacity(32));
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
