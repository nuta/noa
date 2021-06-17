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
    ui::{
        truncate_to_width, Canvas, Compositor, Context, DisplayWidth, Event, HandledEvent, Layout,
        RectSize, Surface,
    },
};

#[derive(Debug)]
pub enum PromptMessage {
    Info(String),
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
    title: String,
    title_width: usize,
    prompt: String,
    prompt_width: usize,
    input_width: usize,
    committed: bool,
    message: Option<PromptMessage>,
    onchange: Option<Box<dyn Fn(&mut Context, &mut LineEdit) -> CallbackResult>>,
    oncommit: Box<dyn Fn(&mut Context, &str) -> CallbackResult>,
}

impl PromptSurface {
    pub fn new(
        ctx: &mut Context,
        title: &str,
        prompt: &str,
        input_width: usize,
        onchange: Option<Box<dyn Fn(&mut Context, &mut LineEdit) -> CallbackResult>>,
        oncommit: Box<dyn Fn(&mut Context, &str) -> CallbackResult>,
    ) -> PromptSurface {
        PromptSurface {
            input: LineEdit::new(),
            title: title.to_owned(),
            title_width: title.display_width(),
            prompt: prompt.to_owned(),
            prompt_width: prompt.display_width(),
            input_width,
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
            width: min(
                screen_size.width,
                max(
                    4 + self.prompt_width + self.input_width,
                    2 + self.title_width,
                ),
            ),
            height: 5,
        };
        (Layout::Center, rect_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        Some((1, 1 + self.input.cursor()))
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        canvas.clear();
        let inner_width = canvas.width() - 2;

        // Title.
        canvas.draw_str(1, 1, truncate_to_width(&self.title, inner_width));

        // Prompt.
        canvas.draw_str(2, 1, &self.prompt);
        canvas.draw_str(2, 1 + self.prompt_width, ": ");
        canvas.draw_str(2, 1 + self.prompt_width + 2, &self.input.text());

        // Message.
        if let Some(message) = &self.message {
            let text = match message {
                PromptMessage::Error(text) => (text),
                PromptMessage::Info(text) => (text),
            };

            canvas.draw_str(3, 1, text);
        }

        // Border.
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
