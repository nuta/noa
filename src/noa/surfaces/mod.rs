use anyhow::Result;
use crossterm::event::KeyEvent;
use noa_buffer::Buffer;

use crate::{
    editor::Editor,
    terminal::{canvas::Canvas, compositor::Compositor},
    view::View,
};

pub mod buffer;

pub struct Context<'a> {
    pub editor: &'a mut Editor,
}

pub trait Surface {
    fn name(&self) -> &str;
    fn invalidated(&self, ctx: &mut Context) -> bool;
    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()>;
    fn handle_key_event(&mut self, ctx: &mut Context, key: KeyEvent) -> Result<()>;
    fn handle_key_batch_event(&mut self, ctx: &mut Context, input: &str) -> Result<()>;
}
