use noa_compositor::{Compositor, Event};
use tokio::sync::oneshot;

use crate::{
    clipboard::{self, ClipboardProvider},
    document::DocumentManager,
    ui::buffer_view::BufferView,
};

pub struct Editor {
    quit: oneshot::Receiver<()>,
    documents: DocumentManager,
    compositor: Compositor,
    clipboard_provider: Box<dyn ClipboardProvider>,
}

impl Editor {
    pub fn new() -> Editor {
        let (quit_tx, quit) = oneshot::channel();
        Editor {
            quit,
            documents: DocumentManager::new(),
            compositor: Compositor::new(),
            clipboard_provider: clipboard::build_provider()
                .unwrap_or_else(clipboard::build_dummy_provider),
        }
    }

    pub async fn run(mut self) {
        self.compositor
            .add_frontmost_layer(Box::new(BufferView::new()), true, 0, 0);

        loop {
            self.compositor.render_to_terminal();

            tokio::select! {
                biased;

                _ = &mut self.quit => {
                    break;
                }

                Some(ev) = self.compositor.recv_terminal_event() => {
                    self.handle_terminal_event(ev);
                }
            }
        }
    }

    fn handle_terminal_event(&mut self, ev: Event) {
        match ev {
            Event::Input(input) => {
                self.compositor.handle_input(input);
            }
            Event::Resize { height, width } => {
                self.compositor.resize_screen(height, width);
            }
        }
    }
}
