use noa_compositor::terminal::Event;
use tokio::sync::oneshot;

use crate::{
    clipboard::{self, ClipboardProvider},
    document::DocumentManager,
    ui::buffer_view::BufferView,
};

pub struct Editor {
    documents: DocumentManager,
    clipboard_provider: Box<dyn ClipboardProvider>,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            documents: DocumentManager::new(),
            clipboard_provider: clipboard::build_provider()
                .unwrap_or_else(clipboard::build_dummy_provider),
        }
    }
}
