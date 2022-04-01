use std::cmp::{max, min};

use noa_compositor::{
    canvas::CanvasViewMut,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind},
    Compositor,
};

use crate::editor::Editor;

pub struct BumpView {
    active: bool,
    text: String,
    lines: Vec<String>,
    scroll: usize,
    height: usize,
    width: usize,
}

impl BumpView {
    pub fn new() -> BumpView {
        BumpView {
            active: false,
            text: String::new(),
            lines: Vec::new(),
            scroll: 0,
            height: 0,
            width: 0,
        }
    }

    pub fn open(&mut self, text: &str) {
        self.active = true;
        self.lines = textwrap::wrap(text, self.width)
            .iter()
            .map(|s| s.to_string())
            .collect();
        self.text = text.to_owned();
        self.scroll = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn page_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(self.height);
    }

    pub fn page_down(&mut self) {
        self.scroll = min(
            self.scroll + self.height,
            self.lines.len().saturating_sub(1),
        );
    }
}

impl Surface for BumpView {
    type Context = Editor;

    fn name(&self) -> &str {
        "bump"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _editor: &mut Editor) -> bool {
        self.active
    }

    fn layout(&mut self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        let height = max(8, screen_size.height / 3);
        (
            Layout::Fixed {
                y: screen_size.height.saturating_sub(height),
                x: 0,
            },
            RectSize {
                height,
                width: screen_size.width,
            },
        )
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, _editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        self.height = canvas.height();
        if self.width != canvas.width() {
            // The screen size has changed.
            self.width = canvas.width();
            self.lines = textwrap::wrap(&self.text, self.width)
                .iter()
                .map(|s| s.to_string())
                .collect();
            self.scroll = min(self.scroll, self.lines.len().saturating_sub(1));
        }

        for (y, line) in self
            .lines
            .iter()
            .skip(self.scroll)
            .take(canvas.height())
            .enumerate()
        {
            canvas.write_str(y, 0, line);
        }
    }

    fn handle_key_event(
        &mut self,
        _editor: &mut Editor,
        _compositor: &mut Compositor<Self::Context>,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        // const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        match (key.code, key.modifiers) {
            (KeyCode::PageUp, NONE) => {
                self.page_up();
            }
            (KeyCode::PageDown, NONE) => {
                self.page_down();
            }
            _ => {
                self.close();
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut Self::Context,
        _compositor: &mut Compositor<Editor>,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }

    fn handle_mouse_event(
        &mut self,
        _ctx: &mut Self::Context,
        _compositor: &mut Compositor<Self::Context>,
        _kind: MouseEventKind,
        _modifiers: KeyModifiers,
        _surface_y: usize,
        _surface_x: usize,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }
}
