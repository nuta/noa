use std::ops::ControlFlow;

use noa_buffer::display_width::DisplayWidth;

use noa_compositor::{
    canvas::CanvasViewMut,
    line_edit::LineEdit,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyEvent, KeyModifiers},
    Compositor,
};

use crate::{
    editor::Editor,
    event_listener::{event_pair, EventListener, EventPair},
    theme::theme_for,
};

use super::helpers::truncate_to_width;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptMode {
    String,
    SingleChar,
}

pub struct PromptView {
    active: bool,
    canceled: bool,
    mode: PromptMode,
    title: String,
    title_width: usize,
    input: LineEdit,
    entered_event: EventPair,
}

impl PromptView {
    pub fn new() -> PromptView {
        PromptView {
            active: false,
            canceled: false,
            mode: PromptMode::String,
            title: "".to_string(),
            title_width: 0,
            input: LineEdit::new(),
            entered_event: event_pair(),
        }
    }

    pub fn input(&self) -> &LineEdit {
        &self.input
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled
    }

    pub fn entered_event_listener(&self) -> &EventListener {
        &self.entered_event.listener
    }

    pub fn activate<S: Into<String>>(&mut self, mode: PromptMode, title: S) {
        self.active = true;
        self.canceled = false;
        self.mode = mode;
        self.title = title.into();
        self.title_width = self.title.display_width();
        self.input.clear();
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.input.clear();
    }

    pub fn clear(&mut self) {
        self.input.clear();
    }
}

impl Surface for PromptView {
    type Context = Editor;

    fn name(&self) -> &str {
        "prompt"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _editor: &mut Editor) -> bool {
        self.active
    }

    fn layout(&mut self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        let height = 1;
        (
            Layout::Fixed {
                y: screen_size.height.saturating_sub(1 + height),
                x: 0,
            },
            RectSize {
                height,
                width: screen_size.width,
            },
        )
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        Some((0, 1 + self.title_width + self.input.cursor_position()))
    }

    fn render(&mut self, _editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        self.input.relocate_scroll(canvas.width());

        let input_x = 1 + self.title.display_width() + 1;

        canvas.write_str_with_style(0, 1, &self.title, theme_for("prompt.name"));
        canvas.write_str(
            0,
            input_x,
            truncate_to_width(&self.input.text(), canvas.width() - input_x),
        );
        canvas.apply_style(0, input_x, canvas.width(), theme_for("prompt.name"));
    }

    fn handle_key_event(
        &mut self,
        _editor: &mut Editor,
        _compositor: &mut Compositor<Self::Context>,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        trace!("prompt: {:?}", key);
        match (key.code, key.modifiers) {
            (KeyCode::Enter, NONE) => {
                self.entered_event.producer.notify_all();
            }
            (KeyCode::Esc, _) | (KeyCode::Char('q'), CTRL) => {
                self.canceled = true;
                self.entered_event.producer.notify_all();
            }
            _ => {
                self.input.consume_key_event(key);
                if self.mode == PromptMode::SingleChar && !self.input.is_empty() {
                    self.entered_event.producer.notify_all();
                }
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _editor: &mut Editor,
        _compositor: &mut Compositor<Editor>,
        input: &str,
    ) -> HandledEvent {
        self.input.insert(&input.replace('\n', " "));
        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        _ctx: &mut Self::Context,
        _compositor: &mut Compositor<Self::Context>,
        _kind: noa_compositor::terminal::MouseEventKind,
        _modifiers: noa_compositor::terminal::KeyModifiers,
        _surface_y: usize,
        _surface_x: usize,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }
}

pub fn prompt<S, F, C>(
    compositor: &mut Compositor<Editor>,
    editor: &mut Editor,
    _mode: PromptMode,
    title: S,
    mut enter_callback: F,
    _completion_callback: C,
) where
    S: Into<String>,
    F: FnMut(&mut Compositor<Editor>, &mut Editor, Option<String>) -> ControlFlow<()>
        + Send
        + 'static,
    C: FnMut(&mut Editor, &LineEdit) -> Option<Vec<String>> + 'static,
{
    let title = title.into();

    let prompt_view: &mut PromptView = compositor.get_mut_surface_by_name("prompt");

    editor.jobs.listen_in_mainloop(
        prompt_view.entered_event_listener().clone(),
        move |editor, compositor| {
            info!("Enter pressed in prompt");
            let prompt_view: &mut PromptView = compositor.get_mut_surface_by_name("prompt");

            let result = if prompt_view.is_canceled() {
                None
            } else {
                Some(prompt_view.input().text())
            };

            match enter_callback(compositor, editor, result) {
                ControlFlow::Continue(()) => {}
                ControlFlow::Break(()) => {
                    let prompt_view: &mut PromptView = compositor.get_mut_surface_by_name("prompt");
                    prompt_view.deactivate();
                }
            }
        },
    );

    // let completion_cb = {
    //     let title = title.clone();
    //     editor.register_callback(move |compositor, editor| {
    //         let prompt_view: &mut PromptView = compositor.get_mut_surface_by_name("prompt");
    //         if let Some(items) = completion_callback(editor, prompt_view.input()) {
    //             // prompt_view.set_completion_items(items);
    //         }
    //     })
    // };

    let prompt_view: &mut PromptView = compositor.get_mut_surface_by_name("prompt");
    prompt_view.activate(PromptMode::SingleChar, title);
}
