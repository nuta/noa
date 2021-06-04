use anyhow::Result;
use crossterm::event::KeyEvent;
use noa_buffer::Buffer;

use crate::{terminal::compositor::Canvas, view::View};

pub mod buffer;

pub struct Context<'a> {
    pub exited: &'a mut bool,
    pub buffer: &'a mut Buffer,
    pub view: &'a mut View,
}

pub trait Surface {
    fn is_invalidated(&self, ctx: &mut Context) -> bool;
    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()>;
    fn handle_key_event(&mut self, ctx: &mut Context, key: KeyEvent) -> Result<()>;
    fn handle_key_batch_event(&mut self, ctx: &mut Context, input: &str) -> Result<()>;
}
