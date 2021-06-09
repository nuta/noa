use anyhow::Result;
use crossterm::event::KeyEvent;
use tokio::sync::mpsc::UnboundedSender;

use crate::editor::Editor;

use super::{Canvas, Compositor, Event};

pub struct Context<'a> {
    pub event_tx: &'a UnboundedSender<Event>,
    pub editor: &'a mut Editor,
}

#[derive(Clone, Copy, Debug)]
pub enum Layout {
    Full,
    Center,
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
    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize);
    /// Returns the cursor position in surface-local `(y, x)`. `None` if the cursor
    /// is hidden.
    fn cursor_position(&self) -> Option<(usize, usize)>;
    /// Renders its contents into the canvas. It may update only updated areas.
    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas);
    /// Render its contents into the canvas. It must fill the whole canvas; the
    /// canvas can be the newly created one due to, for example, screen resizing.
    fn render_all(&mut self, ctx: &mut Context, canvas: &mut Canvas);
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
