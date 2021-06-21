mod canvas;
mod compositor;
mod display_width;
mod surface;
mod theme;

use std::cmp::min;

pub use canvas::{Canvas, CanvasViewMut, DrawOp};
pub use compositor::{Compositor, Event};
pub use display_width::DisplayWidth;
pub use surface::{Context, HandledEvent, Layout, RectSize, Surface};
pub use theme::{Theme, DEFAULT_THEME};

pub fn whitespaces(n: usize) -> String {
    " ".repeat(n)
}

pub fn truncate_to_width(s: &str, width: usize) -> &str {
    // TODO: Support CJK using DisplayWidth
    &s[..min(s.chars().count(), width)]
}
