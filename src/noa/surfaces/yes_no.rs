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

use super::{
    prompt::{CallbackResult, PromptMessage},
    PromptSurface,
};

pub struct YesNoChoice {
    pub key: char,
    pub callback: Box<dyn Fn(&mut Context) -> CallbackResult>,
}

pub struct YesNoSurface {
    prompt: PromptSurface,
}

impl YesNoSurface {
    pub fn new(ctx: &mut Context, title: &str, choices: Vec<YesNoChoice>) -> YesNoSurface {
        let mut keys = String::with_capacity(choices.len());
        for choice in &choices {
            keys.push(choice.key);
        }

        YesNoSurface {
            prompt: PromptSurface::new(
                ctx,
                title,
                &format!("[{}]", keys),
                1,
                Some(Box::new(move |ctx, le| {
                    if le.is_empty() {
                        return CallbackResult::Ok;
                    }

                    let input_char = le.text().chars().next().unwrap();
                    for choice in &choices {
                        if choice.key == input_char {
                            return (choice.callback)(ctx);
                        }
                    }

                    le.clear();

                    CallbackResult::ShowMessage(PromptMessage::Error(format!(
                        "invalid input '{}'",
                        input_char
                    )))
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
