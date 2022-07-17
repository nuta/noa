#[macro_use]
extern crate log;

pub mod canvas;
mod compositor;
pub mod line_edit;
pub mod surface;
pub mod terminal;
pub mod terminal_exts;

pub use compositor::Compositor;
