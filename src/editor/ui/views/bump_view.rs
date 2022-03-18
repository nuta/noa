use std::cmp::{max, min};

use noa_terminal::{
    canvas::CanvasViewMut,
    terminal::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind},
};

use crate::{
    editor::Editor,
    event_listener::{event_pair, EventListener, EventPair},
    notification::{notification_manager, Notification},
    theme::theme_for,
    ui::{
        compositor::Compositor,
        helpers::truncate_to_width,
        line_edit::LineEdit,
        markdown::Markdown,
        surface::{HandledEvent, Layout, RectSize, Surface, UIContext},
    },
};

pub struct BumpView {
    active: bool,
    text: Markdown,
    lines: Vec<String>,
    scroll: usize,
    height: usize,
    width: usize,
}

impl BumpView {
    pub fn new() -> BumpView {
        BumpView {
            active: false,
            text: Markdown::new("".to_string()),
            lines: Vec::new(),
            scroll: 0,
            height: 0,
            width: 0,
        }
    }

    pub fn open(&mut self, text: Markdown) {
        self.active = true;
        self.lines = text.render(self.width);
        self.text = text;
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
    fn name(&self) -> &str {
        "bump"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _ctx: &mut UIContext) -> bool {
        self.active
    }

    fn layout(&mut self, _ctx: &mut UIContext, screen_size: RectSize) -> (Layout, RectSize) {
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

    fn cursor_position(&self, _ctx: &mut UIContext) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, _ctx: &mut UIContext, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        self.height = canvas.height();
        if self.width != canvas.width() {
            // The screen size has changed.
            self.width = canvas.width();
            self.lines = self.text.render(self.width);
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
        _ctx: &mut UIContext,
        _compositor: &mut Compositor,
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
        _ctx: &mut UIContext,
        _compositor: &mut Compositor,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Ignored
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
        HandledEvent::Ignored
    }
}
