use crate::terminal::canvas::Canvas;

use super::{truncate_to_width, Context, Surface};
use anyhow::Result;
use crossterm::event::KeyEvent;

pub struct TooSmallSurface {
    text: String,
}

impl TooSmallSurface {
    pub fn new(text: &str) -> TooSmallSurface {
        TooSmallSurface {
            text: text.to_string(),
        }
    }
}

impl Surface for TooSmallSurface {
    fn name(&self) -> &str {
        "too_small"
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()> {
        self.render_all(ctx, canvas)
    }

    fn render_all(&mut self, _ctx: &mut Context, canvas: &mut Canvas) -> Result<()> {
        canvas.set_str(0, 0, truncate_to_width(&self.text, canvas.width()));
        Ok(())
    }

    fn handle_key_event(&mut self, _ctx: &mut Context, _key: KeyEvent) -> Result<()> {
        Ok(())
    }

    fn handle_key_batch_event(&mut self, _ctx: &mut Context, _input: &str) -> Result<()> {
        Ok(())
    }
}
