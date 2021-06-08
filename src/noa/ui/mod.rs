mod canvas;
mod compositor;
mod display_width;
mod surface;
mod surfaces;

use std::cmp::min;

pub use canvas::{Canvas, DrawOp};
pub use compositor::{Compositor, Event};
pub use display_width::DisplayWidth;
pub use surface::{Context, Layout, RectSize, Surface};

fn whitespaces(n: usize) -> String {
    " ".repeat(n)
}

pub fn truncate_to_width(s: &str, width: usize) -> &str {
    // TODO: Support CJK using DisplayWidth
    &s[..min(s.chars().count(), width)]
}
