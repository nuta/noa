use noa_terminal::{
    canvas::CanvasViewMut,
    terminal::{KeyEvent, KeyModifiers, MouseEventKind},
};

use crate::{
    editor::Editor,
    ui::{
        compositor::Compositor,
        helpers::truncate_to_width,
        surface::{HandledEvent, Layout, RectSize, Surface, UIContext},
    },
};

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
    fn name(&self) -> &str {
        "too_small"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _ctx: &mut UIContext) -> bool {
        true
    }

    fn layout(&mut self, _ctx: &mut UIContext, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Fixed { x: 0, y: 0 }, screen_size)
    }

    fn cursor_position(&self, _ctx: &mut UIContext) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, _ctx: &mut UIContext, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();
        canvas.write_str(0, 0, truncate_to_width(&self.text, canvas.width()));
    }

    fn handle_key_event(
        &mut self,
        _ctx: &mut UIContext,
        _compositor: &mut Compositor,
        _key: KeyEvent,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut UIContext,
        _compositor: &mut Compositor,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        _ctx: &mut UIContext,
        _compositor: &mut Compositor,
        _kind: MouseEventKind,
        _modifiers: KeyModifiers,
        _surface_y: usize,
        _surface_x: usize,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }
}
