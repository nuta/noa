use std::{slice, sync::Arc};

use crossterm::event::{KeyEvent, MouseEvent};
use noa_buffer::Point;
use noa_common::sync_protocol::{FileLocation, Notification};
use noa_common::time_report::TimeReport;
use parking_lot::Mutex;

use crate::ui::{Context, Surface};

use crate::terminal::Terminal;

use super::{truncate_to_width, Canvas, CanvasViewMut, HandledEvent, Layout, RectSize};

#[derive(Debug)]
pub enum Event {
    ReDraw,
    Key(KeyEvent),
    Mouse(MouseEvent),
    KeyBatch(String),
    Resize {
        screen_height: usize,
        screen_width: usize,
    },
    OpenFile(FileLocation),
    Notification(Notification),
}

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
    screens: [Canvas; 2],
    screen_size: RectSize,
    active_screen_index: usize,
    /// The last element comes foreground.
    layers: Vec<Arc<Mutex<Layer>>>,
}

impl Compositor {
    pub fn new(terminal: Terminal) -> Compositor {
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
            surface: Box::new(TooSmallSurface::new("too small!")),
            active: true,
            canvas: Canvas::new(screen_size.height, screen_size.width),
            screen_y: 0,
            screen_x: 0,
        })));

        Compositor {
            terminal,
            layers,
            screens: screen,
            active_screen_index: 0,
            screen_size,
        }
    }

    pub fn push_layer(&mut self, _ctx: &mut Context, surface: impl Surface + 'static) {
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

    pub fn resize_screen(&mut self, _ctx: &mut Context, height: usize, width: usize) {
        self.screen_size = RectSize { height, width };
        self.screens = [Canvas::new(height, width), Canvas::new(height, width)];
        self.terminal.clear();
    }

    pub fn render_to_terminal(&mut self, ctx: &mut Context) {
        // Re-layout layers.
        let cursor_pos = ctx.editor.current_file().read().buffer.main_cursor_pos();
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
        compose_layers(ctx, &mut self.screens[screen_index], self.layers.iter());
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

    pub fn handle_event(&mut self, ctx: &mut Context, ev: Event) {
        match ev {
            Event::Key(key) => {
                for layer_lock in self.layers.clone().iter().rev() {
                    let mut layer = layer_lock.lock();
                    if layer.active {
                        if let HandledEvent::Consumed =
                            layer.surface.handle_key_event(ctx, self, key)
                        {
                            return;
                        }
                    }
                }
            }
            Event::Mouse(ev) => {
                for layer_lock in self.layers.clone().iter().rev() {
                    let mut layer = layer_lock.lock();
                    if layer.active {
                        if let HandledEvent::Consumed =
                            layer.surface.handle_mouse_event(ctx, self, ev)
                        {
                            return;
                        }
                    }
                }
            }
            Event::KeyBatch(input) => {
                for layer_lock in self.layers.clone().iter().rev() {
                    let mut layer = layer_lock.lock();
                    if layer.active {
                        if let HandledEvent::Consumed =
                            layer.surface.handle_key_batch_event(ctx, self, &input)
                        {
                            return;
                        }
                    }
                }
            }
            Event::Resize {
                screen_height,
                screen_width,
            } => {
                self.resize_screen(ctx, screen_height, screen_width);
            }
            Event::OpenFile(loc) => {
                ctx.editor.open_file(&loc.path, Some(loc.pos));
            }
            Event::Notification(noti) => {
                ctx.editor.handle_sync_notification(noti);
            }
            Event::ReDraw => {
                // We have to do nothing here.
            }
        }
    }
}

/// Renders each surfaces and copy the compose into the screen canvas.
fn compose_layers<'a, 'b, 'c>(
    ctx: &'a mut Context,
    screen: &'b mut Canvas,
    layers: slice::Iter<'c, Arc<Mutex<Layer>>>,
) {
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

        trace!("rendering {} layer", layer.surface.name());

        layer.surface.render(ctx, layer.canvas.view_mut());
        screen.copy_from_other(layer.screen_y, layer.screen_x, &layer.canvas);
    }
}

fn relayout_layers(
    screen_size: RectSize,
    surface: &(impl Surface + ?Sized),
    cursor_pos: Point,
) -> ((usize, usize), RectSize) {
    let (layout, rect) = surface.layout(screen_size);

    let (screen_y, screen_x) = match layout {
        Layout::Fixed { y, x } => (y, x),
        Layout::Center => (
            (screen_size.height / 2).saturating_sub(rect.height / 2),
            (screen_size.width / 2).saturating_sub(rect.width / 2),
        ),
        Layout::AroundCursor => {
            let y = if cursor_pos.y + rect.height + 1 > screen_size.height {
                cursor_pos.y.saturating_sub(rect.height + 1)
            } else {
                cursor_pos.y + 1
            };

            let x = if cursor_pos.x + rect.width > screen_size.width {
                cursor_pos.x.saturating_sub(rect.width)
            } else {
                cursor_pos.x
            };

            (y, x)
        }
    };

    ((screen_y, screen_x), rect)
}

struct TooSmallSurface {
    text: String,
}

impl TooSmallSurface {
    pub fn new(text: &str) -> TooSmallSurface {
        TooSmallSurface {
            text: text.to_string(),
        }
    }
}

impl Surface for TooSmallSurface {
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

    fn render<'a>(&mut self, _ctx: &mut Context, mut canvas: CanvasViewMut<'a>) {
        canvas.draw_str(0, 0, truncate_to_width(&self.text, canvas.width()));
    }

    fn handle_key_event(
        &mut self,
        _ctx: &mut Context,
        _compositor: &mut Compositor,
        _key: KeyEvent,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut Context,
        _compositor: &mut Compositor,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Consumed
    }
}
