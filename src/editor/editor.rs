use crate::{
    clipboard::{self, ClipboardProvider},
    document::DocumentManager,
    ui::compositor::Compositor,
};

pub struct Editor {
    documents: DocumentManager,
    compositor: Compositor,
    clipboard_provider: Box<dyn ClipboardProvider>,
}

impl Editor {
    pub fn new(compositor: Compositor) -> Editor {
        let clipboard_provider =
            clipboard::build_provider().unwrap_or_else(clipboard::build_dummy_provider);
        Editor {
            documents: DocumentManager::new(),
            compositor,
            clipboard_provider,
        }
    }

    pub async fn run(self) {
        //
    }
}
