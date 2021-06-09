#[allow(unused_imports)]
#[macro_use]
extern crate log;

mod buffer;
pub mod lang;
mod rope;
mod snapshot;

pub use buffer::{Buffer, BufferId};
pub use lang::Lang;
pub use rope::{Cursor, Point, Range, Rope};
pub use snapshot::Snapshot;
