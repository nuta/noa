use noa_compositor::terminal::Event;
use tokio::sync::oneshot;

use crate::{
    clipboard::{self, ClipboardProvider},
    document::DocumentManager,
    notification::NotificationManager,
    ui::buffer_view::BufferView,
};

pub struct Editor {
    pub documents: DocumentManager,
    pub notifications: NotificationManager,
    pub clipboard_provider: Box<dyn ClipboardProvider>,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            documents: DocumentManager::new(),
            notifications: NotificationManager::new(),
            clipboard_provider: clipboard::build_provider()
                .unwrap_or_else(clipboard::build_dummy_provider),
        }
    }
}
