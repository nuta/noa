use anyhow::Result;
use crossterm::event::KeyEvent;

use crate::ui::{Canvas, Context, Layout, RectSize, Surface};

pub struct Finder {}

impl Surface for Finder {
    fn name(&self) -> &str {
        "finder"
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Center, screen_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        // TODO:
        None
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()> {
        self.render_all(ctx, canvas)
    }

    fn render_all(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()> {
        // TODO:
        Ok(())
    }

    fn handle_key_event(&mut self, ctx: &mut Context, key: KeyEvent) -> Result<()> {
        // TODO:
        Ok(())
    }

    fn handle_key_batch_event(&mut self, ctx: &mut Context, input: &str) -> Result<()> {
        // TODO:
        Ok(())
    }
}
