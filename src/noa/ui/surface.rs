use crossterm::event::{KeyEvent, MouseEvent};
use tokio::sync::mpsc::UnboundedSender;

use crate::editor::Editor;

use super::{canvas::CanvasViewMut, theme::Theme, Compositor, Event};

pub struct Context<'a> {
    pub event_tx: &'a UnboundedSender<Event>,
    pub editor: &'a mut Editor,
    pub theme: &'a Theme,
}

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

#[derive(Clone, Copy, Debug)]
pub enum HandledEvent {
    Consumed,
    Ignored,
}

pub trait Surface {
    fn name(&self) -> &str;
    fn is_visible(&self) -> bool;
    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize);
    /// Returns the cursor position in surface-local `(y, x)`. `None` if the cursor
    /// is hidden.
    fn cursor_position(&self) -> Option<(usize, usize)>;
    /// Render its contents into the canvas. It must fill the whole canvas; the
    /// canvas can be the newly created one due to, for example, screen resizing.
    fn render<'a>(&mut self, _ctx: &mut Context, canvas: CanvasViewMut<'a>);
    fn handle_key_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        key: KeyEvent,
    ) -> HandledEvent;
    fn handle_mouse_event(
        &mut self,
        _ctx: &mut Context,
        _compositor: &mut Compositor,
        _ev: MouseEvent,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }
    fn handle_key_batch_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        input: &str,
    ) -> HandledEvent;
}
