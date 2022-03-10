use std::cmp::min;

use noa_buffer::cursor::Range;
use noa_compositor::{
    canvas::CanvasViewMut,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind},
    Compositor,
};

use crate::{completion::CompletionItem, editor::Editor, theme::theme_for};

use super::helpers::truncate_to_width;

pub struct CompletionView {
    active: bool,
    selected_index: usize,
}

impl CompletionView {
    pub fn new() -> CompletionView {
        CompletionView {
            active: false,
            selected_index: 0,
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if !self.active {
            self.selected_index = 0;
        }
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
        self.active
    }

    fn layout(&mut self, editor: &mut Editor, _screen_size: RectSize) -> (Layout, RectSize) {
        let doc = editor.documents.current();
        let items = doc.completion_items();
        let longest_entry_len = items.iter().map(|e| e.label.len()).max().unwrap_or(0);

        let height = min(6, items.len());
        let width = min(32, longest_entry_len);
        (Layout::AroundCursor, RectSize { height, width })
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        let doc = editor.documents.current();
        for (i, e) in doc
            .completion_items()
            .iter()
            .enumerate()
            .take(canvas.height())
        {
            canvas.write_str(i, 0, truncate_to_width(&e.label, canvas.width()));
            canvas.apply_style(i, 0, canvas.width(), theme_for("completion.item"));
        }
    }

    fn handle_key_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        editor: &mut Editor,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.documents.current_mut();

        if doc.completion_items().is_empty() {
            return HandledEvent::Ignored;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Esc, NONE) => {
                doc.clear_completion_items();
                self.set_active(false);
            }
            (KeyCode::Tab, NONE) => {
                if let Some(item) = doc.completion_items().get(self.selected_index).cloned() {
                    doc.buffer_mut().apply_text_edits(item.text_edits);
                    doc.buffer_mut().save_undo();
                }

                doc.clear_completion_items();
                self.set_active(false);
            }
            (KeyCode::Up, NONE) => {
                // In case the # of items was decreased (I think won't happen though).
                self.selected_index = min(
                    self.selected_index,
                    doc.completion_items().len().saturating_sub(1),
                );

                self.selected_index = self.selected_index.saturating_sub(1);
            }
            (KeyCode::Down, NONE) => {
                self.selected_index = min(
                    self.selected_index + 1,
                    doc.completion_items().len().saturating_sub(1),
                );
            }
            _ => {
                return HandledEvent::Ignored;
            }
        }

        HandledEvent::Consumed
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
