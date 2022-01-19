use crate::{
    clipboard::{self, ClipboardProvider},
    document::DocumentManager,
    ui::Ui,
};

pub struct Editor {
    documents: DocumentManager,
    ui: Ui,
    clipboard_provider: Box<dyn ClipboardProvider>,
}

impl Editor {
    pub fn new(ui: Ui) -> Editor {
        let clipboard_provider =
            clipboard::build_provider().unwrap_or_else(clipboard::build_dummy_provider);
        Editor {
            documents: DocumentManager::new(),
            ui,
            clipboard_provider,
        }
    }

    pub async fn run(self) {
        //
    }
}
