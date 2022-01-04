use crate::document::DocumentManager;

pub struct Editor {
    documents: DocumentManager,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            documents: DocumentManager::new(),
        }
    }

    pub async fn run(self) {
        //
    }
}
