#[macro_use]
extern crate log;

pub mod canvas;
mod compositor;
pub mod surface;
mod terminal;
mod widgets;

pub use compositor::Compositor;
pub use terminal::{Event, InputEvent, Terminal};
