#[macro_use]
extern crate log;

pub mod canvas;
mod compositor;
pub mod surface;
pub mod terminal;
mod widgets;

pub use compositor::Compositor;
