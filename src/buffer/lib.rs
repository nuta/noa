#[allow(unused_imports)]
#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

mod buffer;
mod cursor;
mod rope;
mod snapshot;

pub use buffer::{Buffer, BufferId};
pub use cursor::{Cursor, Point, Range};
pub use rope::{Rope, SearchIter};
pub use snapshot::Snapshot;
