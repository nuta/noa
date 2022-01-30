use std::path::Path;

use anyhow::Result;
use noa_compositor::terminal::Event;
use tokio::sync::oneshot;

use crate::{
    clipboard::{self, ClipboardProvider},
    document::{Document, DocumentManager},
    notification::NotificationManager,
    theme::Theme,
    ui::buffer_view::BufferView,
};

pub struct Editor {
    pub theme: Theme,
    pub documents: DocumentManager,
    pub notifications: NotificationManager,
    pub clipboard: Box<dyn ClipboardProvider>,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            theme: Theme::default(),
            documents: DocumentManager::new(),
            notifications: NotificationManager::new(),
            clipboard: clipboard::build_provider().unwrap_or_else(clipboard::build_dummy_provider),
        }
    }
}
