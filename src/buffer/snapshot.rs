use std::sync::Arc;

pub struct Snapshot {
    text: Arc<String>,
}

impl Snapshot {
    pub fn new(text: Arc<String>) -> Snapshot {
        Snapshot { text }
    }

    pub fn text(&self) -> &str {
        &*self.text
    }
}
