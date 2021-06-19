use crossterm::event::KeyEvent;
use tokio::sync::mpsc::UnboundedSender;

use crate::editor::Editor;

use super::{canvas::CanvasViewMut, Compositor, Event};

pub struct Context<'a> {
    pub event_tx: &'a UnboundedSender<Event>,
    pub editor: &'a mut Editor,
}

#[derive(Clone, Copy, Debug)]
pub enum Layout {
    Full,
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
    fn handle_key_batch_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        input: &str,
    ) -> HandledEvent;
}
