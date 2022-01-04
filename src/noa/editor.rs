use crate::{document::DocumentManager, ui::compositor::Compositor};

pub struct Editor {
    documents: DocumentManager,
    compositor: Compositor,
}

impl Editor {
    pub fn new(compositor: Compositor) -> Editor {
        Editor {
            documents: DocumentManager::new(),
            compositor,
        }
    }

    pub async fn run(self) {
        //
    }
}
