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
use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    fuzzy_set::FuzzySet,
    line_edit::LineEdit,
    selector::Selector,
    ui::{Canvas, Compositor, Context, Event, HandledEvent, Layout, RectSize, Surface},
};

#[derive(Debug)]
pub enum PromptMessage {
    Error(String),
}

#[derive(Debug)]
pub enum CallbackResult {
    ShowMessage(PromptMessage),
    Commit,
    Ok,
}

pub struct PromptSurface {
    input: LineEdit,
    committed: bool,
    message: Option<PromptMessage>,
    onchange: Option<Box<dyn Fn(&mut Context, &mut LineEdit) -> CallbackResult>>,
    oncommit: Box<dyn Fn(&mut Context, &str) -> CallbackResult>,
}

impl PromptSurface {
    pub fn new(
        ctx: &mut Context,
        onchange: Option<Box<dyn Fn(&mut Context, &mut LineEdit) -> CallbackResult>>,
        oncommit: Box<dyn Fn(&mut Context, &str) -> CallbackResult>,
    ) -> PromptSurface {
        PromptSurface {
            input: LineEdit::new(),
            committed: false,
            message: None,
            onchange,
            oncommit,
        }
    }

    fn committed(&self) -> bool {
        self.committed
    }

    fn commit(&mut self, ctx: &mut Context, compositor: &mut Compositor) {
        match (self.oncommit)(ctx, &self.input.text()) {
            CallbackResult::Ok => {
                self.committed = true;
                compositor.pop_layer();
            }
            CallbackResult::ShowMessage(msg) => {
                self.message = Some(msg);
            }
            CallbackResult::Commit => {
                panic!("oncommit hook should never use CallbackResult::Commit");
            }
        }
    }
}

impl Surface for PromptSurface {
    fn name(&self) -> &str {
        "prompt"
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        let rect_size = RectSize {
            width: min(max(screen_size.width, 32), 80),
            height: min(max(screen_size.height, 8), 16),
        };
        (Layout::Center, rect_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        Some((1, 1 + self.input.cursor()))
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        canvas.clear();
        canvas.draw_str(1, 1, &self.input.text());
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

        let prev_query = self.input.rope().clone();
        if !self.input.consume_key_event(key) {
            match (key.modifiers, key.code) {
                (NONE, KeyCode::Esc) => {
                    compositor.pop_layer();
                    return HandledEvent::Consumed;
                }
                (NONE, KeyCode::Enter) => {
                    self.commit(ctx, compositor);
                    return HandledEvent::Consumed;
                }
                _ => {
                    trace!("prompt: unhandled key event: {:?}", key);
                    return HandledEvent::Consumed;
                }
            }
        }

        if &prev_query != self.input.rope() {
            if let Some(onchange) = &self.onchange {
                match onchange(ctx, &mut self.input) {
                    CallbackResult::Ok => {}
                    CallbackResult::ShowMessage(msg) => {
                        self.message = Some(msg);
                    }
                    CallbackResult::Commit => {
                        self.commit(ctx, compositor);
                    }
                }
            }
        }
        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        ctx: &mut Context,
        _compositor: &mut Compositor,
        input: &str,
    ) -> HandledEvent {
        self.input.insert(input);
        HandledEvent::Consumed
    }
}
