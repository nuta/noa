use noa_buffer::display_width::DisplayWidth;
use noa_compositor::{
    canvas::CanvasViewMut,
    line_edit::LineEdit,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyEvent, KeyModifiers},
    Compositor,
};

use crate::{
    editor::{Callback, Editor, OnceCallback},
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
    enter_cb: Option<Callback>,
    input: LineEdit,
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
            enter_cb: None,
        }
    }

    pub fn input(&self) -> &LineEdit {
        &self.input
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled
    }

    pub fn activate<S: Into<String>>(&mut self, mode: PromptMode, title: S, callback: Callback) {
        self.active = true;
        self.canceled = false;
        self.mode = mode;
        self.title = title.into();
        self.title_width = self.title.display_width();
        self.enter_cb = Some(callback);
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
        true
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
        compositor: &mut Compositor<Self::Context>,
        editor: &mut Editor,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        match (key.code, key.modifiers) {
            (KeyCode::Enter, NONE) => {
                if let Some(enter_cb) = self.enter_cb.as_ref() {
                    editor.invoke_callback(compositor, *enter_cb);
                }
            }
            (KeyCode::Esc, _) | (KeyCode::Char('g'), CTRL) => {
                self.canceled = true;
                if let Some(enter_cb) = self.enter_cb.as_ref() {
                    editor.invoke_callback(compositor, *enter_cb);
                }
            }
            _ => {
                self.input.consume_key_event(key);
                if self.mode == PromptMode::SingleChar && !self.input.is_empty() {
                    if let Some(enter_cb) = self.enter_cb.as_ref() {
                        editor.invoke_callback(compositor, *enter_cb);
                    }
                }
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor<Editor>,
        _editor: &mut Editor,
        input: &str,
    ) -> HandledEvent {
        self.input.insert(&input.replace('\n', " "));
        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        _ctx: &mut Self::Context,
        _kind: noa_compositor::terminal::MouseEventKind,
        _modifiers: noa_compositor::terminal::KeyModifiers,
        _surface_y: usize,
        _surface_x: usize,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }
}
