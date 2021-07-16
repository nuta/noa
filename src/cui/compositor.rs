use std::time::Duration;
use std::{slice, sync::Arc};

use crossterm::event::KeyEvent;
use noa_common::time_report::TimeReport;
use parking_lot::Mutex;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::timeout;

use crate::surface::{HandledEvent, Layout, RectSize, Surface};
use crate::{truncate_to_width, Canvas, CanvasViewMut, Input, Terminal};

pub struct Layer {
    pub surface: Box<dyn Surface>,
    /// If it's `false`, the surface won't receive key events.
    pub active: bool,
    pub canvas: Canvas,
    pub screen_y: usize,
    pub screen_x: usize,
}

pub struct Compositor {
    terminal: Terminal,
    input_rx: UnboundedReceiver<Input>,
    input_tx: UnboundedSender<Input>,
    screens: [Canvas; 2],
    screen_size: RectSize,
    active_screen_index: usize,
    /// The last element comes foreground.
    layers: Vec<Arc<Mutex<Layer>>>,
}

impl Compositor {
    pub fn new() -> Compositor {
        let (input_tx, input_rx) = unbounded_channel();
        let terminal = Terminal::new(input_tx.clone());

        let screen_size = RectSize {
            height: terminal.height(),
            width: terminal.width(),
        };

        let screen = [
            Canvas::new(screen_size.height, screen_size.width),
            Canvas::new(screen_size.height, screen_size.width),
        ];

        let mut layers = Vec::with_capacity(16);
        layers.push(Arc::new(Mutex::new(Layer {
            surface: Box::new(TooSmall::new("too small!")),
            active: true,
            canvas: Canvas::new(screen_size.height, screen_size.width),
            screen_y: 0,
            screen_x: 0,
        })));

        Compositor {
            terminal,
            input_rx,
            input_tx,
            layers,
            screens: screen,
            active_screen_index: 0,
            screen_size,
        }
    }

    pub fn input_tx(&self) -> UnboundedSender<Input> {
        self.input_tx.clone()
    }

    pub fn push_layer(&mut self, surface: impl Surface + 'static) {
        self.layers.push(Arc::new(Mutex::new(Layer {
            surface: Box::new(surface),
            active: true,
            canvas: Canvas::new(0, 0),
            screen_x: 0,
            screen_y: 0,
        })));
    }

    pub fn pop_layer(&mut self) {
        self.layers.pop();
    }

    pub fn resize_screen(&mut self, height: usize, width: usize) {
        self.screen_size = RectSize { height, width };
        self.screens = [Canvas::new(height, width), Canvas::new(height, width)];
        self.terminal.clear();
    }

    pub fn render_to_terminal(&mut self, cursor_pos: (usize, usize)) {
        // Re-layout layers.
        for layer in &mut self.layers {
            let mut layer = layer.lock();
            let ((screen_y, screen_x), rect_size) =
                relayout_layers(self.screen_size, &*layer.surface, cursor_pos);
            layer.screen_x = screen_x;
            layer.screen_y = screen_y;
            layer.canvas = Canvas::new(rect_size.height, rect_size.width);
        }

        let prev_screen_index = self.active_screen_index;
        self.active_screen_index = (self.active_screen_index + 1) % self.screens.len();
        let screen_index = self.active_screen_index;

        // Render and composite layers.
        let compose_layers_time = TimeReport::new("compose_layers");
        compose_layers(&mut self.screens[screen_index], self.layers.iter());
        compose_layers_time.report();

        // Get the cursor position.
        let mut cursor = None;
        for layer_lock in self.layers.clone().iter().rev() {
            let layer = layer_lock.lock();
            if layer.active {
                if let Some((y, x)) = layer.surface.cursor_position() {
                    cursor = Some((layer.screen_y + y, layer.screen_x + x));
                    break;
                }
            }
        }

        // Compute diffs.
        let compute_draw_updates_time = TimeReport::new("compute_draw_updates");
        let draw_ops =
            self.screens[screen_index].compute_draw_updates(&self.screens[prev_screen_index]);
        compute_draw_updates_time.report();

        // Write into stdout.
        trace!("draw changes: {} items", draw_ops.len());
        let stdout_write_time = TimeReport::new("stdout_write");
        let mut drawer = self.terminal.drawer();
        for op in draw_ops {
            drawer.draw(&op);
        }

        if let Some((screen_y, screen_x)) = cursor {
            drawer.show_cursor(screen_y, screen_x);
        }

        drawer.flush();
        stdout_write_time.report();
    }

    pub fn handle_event(&mut self, ev: Input) -> bool {
        trace!("UI event: {:?}", ev);
        match ev {
            Input::Key(key) => {
                for layer_lock in self.layers.clone().iter().rev() {
                    let mut layer = layer_lock.lock();
                    if layer.active {
                        if let HandledEvent::Consumed = layer.surface.handle_key_event(self, key) {
                            break;
                        }
                    }
                }
            }
            Input::Mouse(ev) => {
                for layer_lock in self.layers.clone().iter().rev() {
                    let mut layer = layer_lock.lock();
                    if layer.active {
                        if let HandledEvent::Consumed = layer.surface.handle_mouse_event(self, ev) {
                            break;
                        }
                    }
                }
            }
            Input::KeyBatch(input) => {
                for layer_lock in self.layers.clone().iter().rev() {
                    let mut layer = layer_lock.lock();
                    if layer.active {
                        if let HandledEvent::Consumed =
                            layer.surface.handle_key_batch_event(self, &input)
                        {
                            break;
                        }
                    }
                }
            }
            Input::Resize {
                screen_height,
                screen_width,
            } => {
                self.resize_screen(screen_height, screen_width);
            }
            Input::Redraw => {}
            Input::Quit => return false,
        }

        true
    }

    pub async fn mainloop<F1, F2, F3, F4, T>(
        &mut self,
        before_event: F1,
        after_event: F2,
        on_idle: F3,
        cursor_pos: F4,
    ) where
        F1: Fn() -> T,
        F2: Fn(T),
        F3: Fn(),
        F4: Fn() -> (usize, usize),
    {
        let mut any_progress = false;
        let mut idle = false;
        loop {
            self.render_to_terminal(cursor_pos());

            let timeout_value = if idle {
                Duration::from_secs(u64::MAX)
            } else {
                Duration::from_millis(300)
            };

            match timeout(timeout_value, self.input_rx.recv()).await {
                Ok(Some(ev)) => {
                    let prev = before_event();

                    any_progress = !matches!(ev, Input::Redraw);
                    if !self.handle_event(ev) {
                        return;
                    }

                    while let Ok(Some(ev)) =
                        timeout(Duration::from_micros(50), self.input_rx.recv()).await
                    {
                        any_progress = any_progress || !matches!(ev, Input::Redraw);
                        if !self.handle_event(ev) {
                            return;
                        }
                    }

                    after_event(prev);
                    idle = false;
                }
                Ok(None) => {
                    break;
                }
                Err(_) if !idle && any_progress => {
                    on_idle();
                    idle = true;
                }
                Err(_) if !any_progress => {
                    idle = true;
                }
                Err(_) => {}
            }
        }
    }
}

/// Renders each surfaces and copy the compose into the screen canvas.
fn compose_layers<'a, 'b>(screen: &'a mut Canvas, layers: slice::Iter<'b, Arc<Mutex<Layer>>>) {
    screen.view_mut().clear();

    for layer in layers {
        let mut layer = layer.lock();
        let layer = &mut *layer;

        if !layer.surface.is_visible() {
            continue;
        }

        // Handle the case when the screen is too small.
        let too_small = screen.width() < 10 || screen.height() < 5;
        let is_too_small_layer = layer.surface.name() == "too_small";
        match (too_small, is_too_small_layer) {
            (true, true) => {}   /* render too_small layer */
            (false, false) => {} /* render layers except too_small */
            _ => continue,
        }

        layer.surface.render(layer.canvas.view_mut());
        screen.copy_from_other(layer.screen_y, layer.screen_x, &layer.canvas);
    }
}

fn relayout_layers(
    screen_size: RectSize,
    surface: &(impl Surface + ?Sized),
    cursor_pos: (usize, usize),
) -> ((usize, usize), RectSize) {
    let (cursor_y, cursor_x) = cursor_pos;
    let (layout, rect) = surface.layout(screen_size);

    let (screen_y, screen_x) = match layout {
        Layout::Fixed { y, x } => (y, x),
        Layout::Center => (
            (screen_size.height / 2).saturating_sub(rect.height / 2),
            (screen_size.width / 2).saturating_sub(rect.width / 2),
        ),
        Layout::AroundCursor => {
            let y = if cursor_y + rect.height + 1 > screen_size.height {
                cursor_y.saturating_sub(rect.height + 1)
            } else {
                cursor_y + 1
            };

            let x = if cursor_x + rect.width > screen_size.width {
                cursor_x.saturating_sub(rect.width)
            } else {
                cursor_x
            };

            (y, x)
        }
    };

    ((screen_y, screen_x), rect)
}

struct TooSmall {
    text: String,
}

impl TooSmall {
    pub fn new(text: &str) -> TooSmall {
        TooSmall {
            text: text.to_string(),
        }
    }
}

impl Surface for TooSmall {
    fn name(&self) -> &str {
        "too_small"
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Fixed { x: 0, y: 0 }, screen_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        None
    }

    fn render<'a>(&mut self, mut canvas: CanvasViewMut<'a>) {
        canvas.draw_str(0, 0, truncate_to_width(&self.text, canvas.width()));
    }

    fn handle_key_event(&mut self, _compositor: &mut Compositor, _key: KeyEvent) -> HandledEvent {
        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }
}
