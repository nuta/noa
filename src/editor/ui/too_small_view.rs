use noa_compositor::{
    canvas::CanvasViewMut,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyEvent, KeyModifiers, MouseEventKind},
    Compositor,
};

use crate::editor::Editor;

use super::helpers::truncate_to_width;

pub struct TooSmallView {
    text: String,
}

impl TooSmallView {
    pub fn new(text: &str) -> TooSmallView {
        TooSmallView {
            text: text.to_string(),
        }
    }
}

impl Surface for TooSmallView {
    type Context = Editor;

    fn name(&self) -> &str {
        "too_small"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _editor: &mut Editor) -> bool {
        true
    }

    fn layout(&mut self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Fixed { x: 0, y: 0 }, screen_size)
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, _editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();
        canvas.write_str(0, 0, truncate_to_width(&self.text, canvas.width()));
    }

    fn handle_key_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        _editor: &mut Editor,
        _key: KeyEvent,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor<Editor>,
        _ctx: &mut Self::Context,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        _ctx: &mut Self::Context,
        _kind: MouseEventKind,
        _modifiers: KeyModifiers,
        _surface_y: usize,
        _surface_x: usize,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }
}
