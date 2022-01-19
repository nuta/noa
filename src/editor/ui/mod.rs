use noa_compositor::{Compositor, Input, Terminal};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

#[derive(Clone, PartialEq, Debug)]
pub enum UiRequest {
    Resize { height: usize, width: usize },
    Input(Input),
}

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
        tokio::task::spawn_blocking(move || {
            while let Some(req) = request_rx.blocking_recv() {
                match req {
                    UiRequest::Input(input) => {
                        compositor.handle_event(input);
                    }
                    UiRequest::Resize { height, width } => {
                        compositor.resize_screen(height, width);
                    }
                }
            }
        });

        Ui { request_tx }
    }

    pub fn send_request(&self, req: UiRequest) {
        self.request_tx.send(req);
    }
}
