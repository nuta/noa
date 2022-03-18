use std::any::Any;

use crate::editor::Editor;

use noa_terminal::{
    canvas::CanvasViewMut,
    terminal::{KeyEvent, KeyModifiers, MouseEventKind},
};

use super::compositor::Compositor;

#[derive(Clone, Copy, Debug)]
pub enum Layout {
    Fixed { y: usize, x: usize },
    Center,
    AroundCursor,
}

#[derive(Clone, Copy, Debug)]
pub struct RectSize {
    pub height: usize,
    pub width: usize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HandledEvent {
    Consumed,
    Ignored,
}

pub struct UIContext<'a> {
    pub editor: &'a mut Editor,
}

pub trait Surface: Any {
    fn name(&self) -> &str;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn is_active(&self, ctx: &mut UIContext) -> bool;
    fn layout(&mut self, ctx: &mut UIContext, screen_size: RectSize) -> (Layout, RectSize);
    /// Returns the cursor position in surface-local `(y, x)`. `None` if the cursor
    /// is hidden.
    fn cursor_position(&self, ctx: &mut UIContext) -> Option<(usize, usize)>;
    /// Render its contents into the canvas. It must fill the whole canvas; the
    /// canvas can be the newly created one due to, for example, screen resizing.
    fn render(&mut self, ctx: &mut UIContext, canvas: &mut CanvasViewMut<'_>);

    fn handle_key_event(
        &mut self,
        _ctx: &mut UIContext,
        _compositor: &mut Compositor,
        _key: KeyEvent,
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
    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut UIContext,
        _compositor: &mut Compositor,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }
}
