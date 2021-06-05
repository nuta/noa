use std::cmp::min;

use anyhow::Result;
use crossterm::event::KeyEvent;
use noa_buffer::Buffer;

use crate::{
    editor::Editor,
    terminal::{canvas::Canvas, compositor::Compositor},
    view::View,
};

pub mod buffer;
pub mod too_small;

pub struct Context<'a> {
    pub editor: &'a mut Editor,
}

pub trait Surface {
    fn name(&self) -> &str;
    /// Renders its contents into the canvas. It may update only updated areas.
    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()>;
    /// Render its contents into the canvas. It must fill the whole canvas; the
    /// canvas can be the newly created one due to, for example, screen resizing.
    fn render_all(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()>;
    fn handle_key_event(&mut self, ctx: &mut Context, key: KeyEvent) -> Result<()>;
    fn handle_key_batch_event(&mut self, ctx: &mut Context, input: &str) -> Result<()>;
}

fn whitespaces(n: usize) -> String {
    " ".repeat(n)
}

pub fn truncate_to_width(s: &str, width: usize) -> &str {
    &s[..min(s.chars().count(), width)]
}
