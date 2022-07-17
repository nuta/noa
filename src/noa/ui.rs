use crate::editor::Editor;

pub struct Ui {
    editor: Editor,
}

impl Ui {
    pub fn new(editor: Editor) -> Self {
        Ui {
            editor,
        }
    }

    pub fn run(self) {

    }
}
