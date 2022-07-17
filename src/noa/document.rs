use std::sync::atomic::{Ordering, AtomicUsize};

use noa_buffer::buffer::Buffer;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentId(usize);

impl DocumentId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        DocumentId(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }
}

pub struct Document {
    pub id: DocumentId,
    buffer: Buffer,
}

impl Document {
    pub fn new() -> Self {
        Document {
            id: DocumentId::new(),
            buffer: Buffer::new(),
        }
    }
}
