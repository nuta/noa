use crate::{CanvasViewMut, Compositor, HandledEvent, KeyEvent, Layout, RectSize, Surface};

use super::prompt::{CallbackResult, Prompt, PromptMessage};

pub struct YesNoChoice {
    pub key: char,
    pub callback: Box<dyn Fn() -> CallbackResult>,
}

impl YesNoChoice {
    pub fn new(key: char, callback: impl Fn() -> CallbackResult + 'static) -> YesNoChoice {
        YesNoChoice {
            key,
            callback: Box::new(callback),
        }
    }
}

pub struct YesNoPrompt {
    prompt: Prompt,
}

impl YesNoPrompt {
    pub fn new(title: &str, choices: Vec<YesNoChoice>) -> YesNoPrompt {
        let mut keys = String::with_capacity(choices.len());
        for choice in &choices {
            keys.push(choice.key);
        }

        YesNoPrompt {
            prompt: Prompt::new(
                title,
                &format!("[{}]", keys),
                1,
                Some(Box::new(move |le| {
                    if le.is_empty() {
                        return CallbackResult::Keep(None);
                    }

                    let input_char = le.text().chars().next().unwrap();
                    for choice in &choices {
                        if choice.key == input_char {
                            return (choice.callback)();
                        }
                    }

                    le.clear();
                    let msg = format!("invalid choice '{}'", input_char);
                    CallbackResult::Keep(Some(PromptMessage::Error(msg)))
                })),
                Box::new(|__input| CallbackResult::Close),
            ),
        }
    }
}

impl Surface for YesNoPrompt {
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

    fn render<'a>(&mut self, canvas: CanvasViewMut<'a>) {
        self.prompt.render(canvas)
    }

    fn handle_key_event(&mut self, compositor: &mut Compositor, key: KeyEvent) -> HandledEvent {
        self.prompt.handle_key_event(compositor, key)
    }

    fn handle_key_batch_event(&mut self, compositor: &mut Compositor, input: &str) -> HandledEvent {
        self.prompt.handle_key_batch_event(compositor, input)
    }
}
