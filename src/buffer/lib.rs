#[allow(unused_imports)]
#[macro_use]
extern crate log;

mod buffer;
mod rope;

pub use buffer::{Buffer, BufferId};
pub use rope::{Cursor, Point, Range, Rope};
