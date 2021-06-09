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
            }
            (NONE, KeyCode::Up) => {
                self.selector.lock().select_prev();
            }
            (NONE, KeyCode::Down) => {
                self.selector.lock().select_next();
            }
            (NONE, KeyCode::Enter) => {}
            _ => {
                return HandledEvent::Ignored;
            }
        };

        HandledEvent::Consumed
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
