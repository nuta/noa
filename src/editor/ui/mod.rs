use noa_compositor::{surface::Surface, Compositor, Input, Terminal};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub enum UiRequest {
    Quit,
    Resize {
        height: usize,
        width: usize,
    },
    AddLayer {
        surface: Box<dyn Surface + Send>,
        active: bool,
        screen_y: usize,
        screen_x: usize,
    },
    Input(Input),
}

#[derive(Clone)]
pub struct Ui {
    request_tx: UnboundedSender<UiRequest>,
}

impl Ui {
    pub fn new() -> Ui {
        let (request_tx, mut request_rx) = unbounded_channel();

        let terminal = Terminal::new({
            let request_tx = request_tx.clone();
            move |ev| {
                request_tx.send(UiRequest::Input(ev));
            }
        });

        // Spawn the UI thread.
        let mut compositor = Compositor::new(terminal);
        compositor.render_to_terminal();
        tokio::task::spawn_blocking(move || {
            while let Some(req) = request_rx.blocking_recv() {
                match req {
                    UiRequest::Input(input) => {
                        compositor.handle_input(input);
                    }
                    UiRequest::Resize { height, width } => {
                        compositor.resize_screen(height, width);
                    }
                    UiRequest::AddLayer {
                        surface,
                        active,
                        screen_y,
                        screen_x,
                    } => {
                        compositor.add_frontmost_layer(surface, active, screen_y, screen_x);
                    }
                }
            }
        });

        Ui { request_tx }
    }

    pub fn quit(&self) {
        self.request_tx.send(UiRequest::Quit);
    }

    pub fn push_layer(
        &self,
        surface: impl Surface + Send + 'static,
        active: bool,
        screen_y: usize,
        screen_x: usize,
    ) {
        self.request_tx.send(UiRequest::AddLayer {
            surface: Box::new(surface),
            active,
            screen_y,
            screen_x,
        });
    }
}
