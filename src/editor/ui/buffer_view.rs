use noa_compositor::{
    canvas::CanvasViewMut,
    surface::{HandledEvent, KeyEvent, Layout, RectSize, Surface},
};

pub struct BufferView {
    /// `(y, x)`.
    cursor_position: (usize, usize),
}

impl BufferView {
    pub fn new() -> BufferView {
        BufferView {
            cursor_position: (0, 0),
        }
    }
}

impl Surface for BufferView {
    fn name(&self) -> &str {
        "buffer"
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Fixed { y: 0, x: 0 }, screen_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        Some(self.cursor_position)
    }

    fn render<'a>(&mut self, mut canvas: CanvasViewMut<'a>) {
        canvas.clear();
    }

    fn handle_key_event(&mut self, ev: KeyEvent) -> HandledEvent {
        todo!()
    }

    fn handle_key_batch_event(&mut self, s: &str) -> HandledEvent {
        todo!()
    }

    fn handle_mouse_event(&mut self, _ev: noa_compositor::surface::MouseEvent) -> HandledEvent {
        todo!()
    }
}
