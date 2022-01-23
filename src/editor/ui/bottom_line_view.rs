use noa_buffer::display_width::DisplayWidth;
use noa_compositor::{
    canvas::{CanvasViewMut, Decoration, Style},
    surface::{HandledEvent, KeyEvent, Layout, MouseEvent, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers},
};
use tokio::{sync::oneshot, task};

use crate::{
    clipboard::{ClipboardData, SystemClipboardData},
    editor::Editor,
};

pub struct BottomLineView {}

impl BottomLineView {
    pub fn new() -> BottomLineView {
        BottomLineView {}
    }
}

impl Surface for BottomLineView {
    type Context = Editor;

    fn name(&self) -> &str {
        "bottom_line"
    }

    fn is_visible(&self, _editor: &mut Editor) -> bool {
        true
    }

    fn layout(&self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        (
            Layout::Fixed {
                y: screen_size.height.saturating_sub(2),
                x: 0,
            },
            RectSize {
                height: 2,
                width: screen_size.width,
            },
        )
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        let doc = editor.documents.current();
        let buffer = doc.buffer();
    }

    fn handle_key_event(&mut self, editor: &mut Editor, key: KeyEvent) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let mut _doc = editor.documents.current_mut();

        match (key.code, key.modifiers) {
            _ => HandledEvent::Ignored,
        }
    }

    fn handle_key_batch_event(&mut self, editor: &mut Editor, s: &str) -> HandledEvent {
        HandledEvent::Ignored
    }

    fn handle_mouse_event(&mut self, editor: &mut Editor, _ev: MouseEvent) -> HandledEvent {
        HandledEvent::Ignored
    }
}
