use std::cmp::min;

use noa_compositor::{
    canvas::CanvasViewMut,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyEvent, KeyModifiers, MouseEventKind},
    Compositor,
};

use crate::{editor::Editor, theme::theme_for};

use super::helpers::truncate_to_width;

pub struct CompletionView {}

impl CompletionView {
    pub fn new() -> CompletionView {
        CompletionView {}
    }
}

impl Surface for CompletionView {
    type Context = Editor;

    fn name(&self) -> &str {
        "completion"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _editor: &mut Editor) -> bool {
        true
    }

    fn layout(&self, editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        let doc = editor.documents.current();
        let entries = doc.completion().entries();
        let longest_entry_len = entries
            .iter()
            .map(|e| e.insert_text.len())
            .max()
            .unwrap_or(0);

        let height = min(6, entries.len());
        let width = min(32, longest_entry_len);
        (Layout::AroundCursor, RectSize { height, width })
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        let doc = editor.documents.current();
        let entries = doc.completion().entries();
        for (i, e) in entries.iter().enumerate().take(canvas.height()) {
            canvas.write_str(i, 0, truncate_to_width(&e.insert_text, canvas.width()));
            canvas.apply_style(i, 0, canvas.width(), theme_for("completion.item"));
        }
    }

    fn handle_key_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        _editor: &mut Editor,
        _key: KeyEvent,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor<Editor>,
        _ctx: &mut Self::Context,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Ignored
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
        HandledEvent::Ignored
    }
}
