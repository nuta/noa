use noa_compositor::compositor::Compositor;

use crate::editor::Editor;

pub struct Ui {
    compositor: Compositor,
    editor: Editor,
}

impl Ui {
    pub fn new(editor: Editor) -> Self {
        Ui {
            compositor: Compositor::new(),
            editor,
        }
    }

    pub async fn run(mut self) {
        self.compositor.render_to_terminal();
        loop {
            tokio::select! {
                biased;

                Some(ev) = self.compositor.recv_terminal_event() => {
                    break;
                }
            }
        }
    }
}
