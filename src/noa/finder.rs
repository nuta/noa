use std::cmp::max;

use anyhow::Result;
use crossterm::event::KeyEvent;

use crate::ui::{Canvas, Compositor, Context, Layout, RectSize, Surface};

pub struct Finder {}

impl Surface for Finder {
    fn name(&self) -> &str {
        "finder"
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        let rect_size = RectSize {
            width: max(screen_size.width, 32),
            height: max(screen_size.height, 8),
        };
        (Layout::Center, rect_size)
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

    fn handle_key_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        key: KeyEvent,
    ) -> Result<()> {
        // TODO:
        Ok(())
    }

    fn handle_key_batch_event(
        &mut self,
        ctx: &mut Context,
        _compositor: &mut Compositor,
        input: &str,
    ) -> Result<()> {
        // TODO:
        Ok(())
    }
}
