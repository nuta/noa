use anyhow::Result;
use crossterm::event::KeyEvent;

use crate::editor::Editor;

use super::{Canvas, Compositor};

pub struct Context<'a> {
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

pub trait Surface {
    fn name(&self) -> &str;
    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize);
    /// Returns the cursor position in surface-local `(y, x)`. `None` if the cursor
    /// is hidden.
    fn cursor_position(&self) -> Option<(usize, usize)>;
    /// Renders its contents into the canvas. It may update only updated areas.
    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()>;
    /// Render its contents into the canvas. It must fill the whole canvas; the
    /// canvas can be the newly created one due to, for example, screen resizing.
    fn render_all(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()>;
    fn handle_key_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        key: KeyEvent,
    ) -> Result<()>;
    fn handle_key_batch_event(&mut self, ctx: &mut Context, input: &str) -> Result<()>;
}
