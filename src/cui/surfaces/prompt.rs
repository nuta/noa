use std::cmp::{max, min};

use crate::{
    truncate_to_width, CanvasViewMut, Compositor, Decoration, DisplayWidth, HandledEvent, KeyCode,
    KeyEvent, KeyModifiers, Layout, LineEdit, RectSize, Surface,
};

#[derive(Debug)]
pub enum PromptMessage {
    Error(String),
}

#[derive(Debug)]
pub enum CallbackResult {
    Keep(Option<PromptMessage>),
    _Commit,
    Close,
}

pub struct Prompt {
    input: LineEdit,
    title: String,
    title_width: usize,
    prompt: String,
    prompt_width: usize,
    input_width: usize,
    message: Option<PromptMessage>,
    onchange: Option<Box<dyn Fn(&mut LineEdit) -> CallbackResult>>,
    oncommit: Box<dyn Fn(&str) -> CallbackResult>,
}

impl Prompt {
    pub fn new(
        title: &str,
        prompt: &str,
        input_width: usize,
        onchange: Option<Box<dyn Fn(&mut LineEdit) -> CallbackResult>>,
        oncommit: Box<dyn Fn(&str) -> CallbackResult>,
    ) -> Prompt {
        Prompt {
            input: LineEdit::new(),
            title: title.to_owned(),
            title_width: title.display_width(),
            prompt: prompt.to_owned(),
            prompt_width: prompt.display_width(),
            input_width,
            message: None,
            onchange,
            oncommit,
        }
    }

    fn commit(&mut self, compositor: &mut Compositor) {
        match (self.oncommit)(&self.input.text()) {
            CallbackResult::Keep(message) => {
                self.message = message;
            }
            CallbackResult::Close => {
                compositor.pop_layer();
            }
            CallbackResult::_Commit => {
                panic!("oncommit hook should never use CallbackResult::Commit");
            }
        }
    }
}

impl Surface for Prompt {
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
            height: 7,
        };
        (Layout::Center, rect_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        Some((
            3,
            1 + self.prompt_width + 2 + self.input.cursor_display_pos(),
        ))
    }

    fn render<'a>(&mut self, mut canvas: CanvasViewMut<'a>) {
        canvas.clear();

        // Border.
        let mut inner = canvas.draw_borders(0, 0, canvas.height(), canvas.width());

        // Title.
        inner.draw_str(0, 0, truncate_to_width(&self.title, inner.width()));
        inner.set_deco(0, 0, inner.width(), Decoration::bold());

        // Prompt.
        inner.draw_str(2, 0, &self.prompt);
        inner.draw_str(2, self.prompt_width, ": ");
        inner.draw_str(2, self.prompt_width + 2, &self.input.text());

        // Message.
        if let Some(message) = &self.message {
            let PromptMessage::Error(text) = message;

            canvas.draw_str(4, 0, text);
        }
    }

    fn handle_key_event(&mut self, compositor: &mut Compositor, key: KeyEvent) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        // const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let prev_query = self.input.rope().clone();
        if self.input.consume_key_event(key) {
            self.input.relocate_scroll(self.input_width);
        } else {
            match (key.modifiers, key.code) {
                (NONE, KeyCode::Esc) => {
                    compositor.pop_layer();
                    return HandledEvent::Consumed;
                }
                (NONE, KeyCode::Enter) => {
                    self.commit(compositor);
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
                match onchange(&mut self.input) {
                    CallbackResult::Keep(message) => {
                        self.message = message;
                    }
                    CallbackResult::Close => {
                        compositor.pop_layer();
                    }
                    CallbackResult::_Commit => {
                        self.commit(compositor);
                    }
                }
            }
        }
        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,

        _compositor: &mut Compositor,
        input: &str,
    ) -> HandledEvent {
        self.input.insert(input);
        self.input.relocate_scroll(self.input_width);
        HandledEvent::Consumed
    }
}
