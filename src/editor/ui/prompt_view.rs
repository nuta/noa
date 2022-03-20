use noa_buffer::display_width::DisplayWidth;

use noa_compositor::{
    canvas::CanvasViewMut,
    line_edit::LineEdit,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyEvent, KeyModifiers},
    Compositor,
};

use crate::{config::theme_for, editor::Editor};

use super::helpers::truncate_to_width;

pub type UpdatedCallback =
    dyn Fn(&mut Editor, &mut Compositor<Editor>, &mut PromptView, bool /* entered */) + Send;

pub struct PromptView {
    active: bool,
    title: String,
    title_width: usize,
    input: LineEdit,
    updated_callback: Option<Box<UpdatedCallback>>,
}

impl PromptView {
    pub fn new() -> PromptView {
        PromptView {
            active: false,
            title: "".to_string(),
            title_width: 0,
            input: LineEdit::new(),
            updated_callback: None,
        }
    }

    pub fn open<S: Into<String>>(&mut self, title: S, updated: Box<UpdatedCallback>) {
        self.active = true;
        self.title = title.into();
        self.title_width = self.title.display_width();
        self.input.clear();
        self.updated_callback = Some(updated);
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn text(&self) -> String {
        self.input.text()
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
        editor: &mut Editor,
        compositor: &mut Compositor<Self::Context>,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        trace!("prompt: {:?}", key);
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('q'), CTRL) => {
                self.close();
            }
            (KeyCode::Enter, NONE) => {
                if let Some(updated) = self.updated_callback.take() {
                    updated(editor, compositor, self, true);
                    self.updated_callback = Some(updated);
                }
            }
            _ => {
                self.input.consume_key_event(key);
                if let Some(updated) = self.updated_callback.take() {
                    updated(editor, compositor, self, false);
                    self.updated_callback = Some(updated);
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
