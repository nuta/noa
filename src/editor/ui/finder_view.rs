use noa_compositor::{
    canvas::CanvasViewMut,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::KeyEvent,
};

use crate::editor::Editor;

struct FinderView {
    text: String,
}

impl FinderView {
    pub fn new(text: &str) -> FinderView {
        FinderView {
            text: text.to_string(),
        }
    }
}

impl Surface for FinderView {
    type Context = Editor;

    fn name(&self) -> &str {
        "finder"
    }

    fn is_visible(&self, _editor: &mut Editor) -> bool {
        true
    }

    fn layout(&self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Fixed { x: 0, y: 0 }, screen_size)
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, _editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {}

    fn handle_key_event(&mut self, _editor: &mut Editor, _key: KeyEvent) -> HandledEvent {
        HandledEvent::Ignored
    }

    fn handle_key_batch_event(&mut self, _editor: &mut Editor, _input: &str) -> HandledEvent {
        HandledEvent::Ignored
    }
}
