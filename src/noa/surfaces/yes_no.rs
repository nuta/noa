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

use super::{prompt::CallbackResult, PromptSurface};

pub struct YesNoSurface {
    prompt: PromptSurface,
}

impl YesNoSurface {
    pub fn new(ctx: &mut Context) -> YesNoSurface {
        YesNoSurface {
            prompt: PromptSurface::new(
                ctx,
                Some(Box::new(|ctx, le| {
                    //
                    CallbackResult::Ok
                })),
                Box::new(|ctx, input| CallbackResult::Ok),
            ),
        }
    }
}

impl Surface for YesNoSurface {
    fn name(&self) -> &str {
        "yes_no"
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        self.prompt.layout(screen_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        self.prompt.cursor_position()
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        self.prompt.render(ctx, canvas)
    }

    fn handle_key_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        key: KeyEvent,
    ) -> HandledEvent {
        self.prompt.handle_key_event(ctx, compositor, key)
    }

    fn handle_key_batch_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        input: &str,
    ) -> HandledEvent {
        self.prompt.handle_key_batch_event(ctx, compositor, input)
    }
}
