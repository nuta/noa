use std::slice;

use noa_common::{oops::OopsExt, time_report::TimeReport};
use tokio::sync::mpsc;

use crate::{canvas::DrawOp, surface::HandledEvent, terminal::InputEvent};

use super::{
    canvas::Canvas,
    surface::{Layout, RectSize, Surface},
    terminal::{self, Terminal},
};

pub struct Layer<C> {
    pub surface: Box<dyn Surface<Context = C> + Send>,
    pub canvas: Canvas,
    pub screen_y: usize,
    pub screen_x: usize,
}

pub struct Compositor<C> {
    terminal: Terminal,
    term_rx: mpsc::UnboundedReceiver<terminal::Event>,
    renderer_tx: mpsc::UnboundedSender<(Vec<DrawOp>, Option<(usize, usize)>)>,
    screens: [Canvas; 2],
    screen_size: RectSize,
    active_screen_index: usize,
    /// The last element comes foreground.
    layers: Vec<Layer<C>>,
    /// A temporary vec to avoid mutual borrowing of self.
    past_layers: Vec<Layer<C>>,
}

#[allow(clippy::new_without_default)]
impl<C> Compositor<C> {
    pub fn new() -> Compositor<C> {
        let (term_tx, term_rx) = mpsc::unbounded_channel();
        let mut terminal = Terminal::new(move |ev| {
            term_tx.send(ev).ok();
        });

        let screen_size = RectSize {
            height: terminal.height(),
            width: terminal.width(),
        };

        let mut drawer = terminal.drawer();
        let (renderer_tx, mut renderer_rx) =
            mpsc::unbounded_channel::<(Vec<DrawOp>, Option<(usize, usize)>)>();
        tokio::task::spawn_blocking(move || {
            while let Some((draw_ops, cursor)) = renderer_rx.blocking_recv() {
                let _drawer_time = TimeReport::new("drawer time");
                drawer.before_drawing();

                trace!("draw changes: {} items", draw_ops.len());
                for op in draw_ops {
                    drawer.draw(&op);
                }
                if let Some((screen_y, screen_x)) = cursor {
                    drawer.move_cursor(screen_y, screen_x);
                }

                drawer.flush();
            }
        });

        Compositor {
            terminal,
            term_rx,
            renderer_tx,
            screens: [
                Canvas::new(screen_size.height, screen_size.width),
                Canvas::new(screen_size.height, screen_size.width),
            ],
            screen_size,
            active_screen_index: 0,
            layers: Vec::new(),
            past_layers: Vec::new(),
        }
    }

    pub async fn recv_terminal_event(&mut self) -> Option<terminal::Event> {
        self.term_rx.recv().await
    }

    pub fn add_frontmost_layer(&mut self, surface: Box<dyn Surface<Context = C> + Send>) {
        debug_assert!(self
            .layers
            .iter()
            .all(|l| l.surface.name() != surface.name()));

        self.layers.push(Layer {
            surface,
            canvas: Canvas::new(0, 0),
            screen_x: 0,
            screen_y: 0,
        });
    }

    pub fn get_mut_surface_by_name<S>(&mut self, name: &str) -> &mut S
    where
        S: Surface<Context = C>,
    {
        for layer in self.layers.iter_mut() {
            if layer.surface.name() == name {
                return layer
                    .surface
                    .as_any_mut()
                    .downcast_mut::<S>()
                    .expect("surface type mismatch");
            }
        }

        for layer in self.past_layers.iter_mut() {
            if layer.surface.name() == name {
                return layer
                    .surface
                    .as_any_mut()
                    .downcast_mut::<S>()
                    .expect("surface type mismatch");
            }
        }

        unreachable!("surface {} not found", name);
    }

    pub fn resize_screen(&mut self, height: usize, width: usize) {
        self.screen_size = RectSize { height, width };
        self.screens = [Canvas::new(height, width), Canvas::new(height, width)];
        self.terminal.clear();
    }

    pub fn render_to_terminal(&mut self, ctx: &mut C) {
        let _rendering_time = TimeReport::new("rendering time");

        // Re-layout layers.
        let mut prev_cursor_pos = None;
        for layer in self.layers.iter_mut() {
            let ((screen_y, screen_x), rect_size) =
                relayout_layer(ctx, &mut *layer.surface, self.screen_size, prev_cursor_pos);
            layer.screen_x = screen_x;
            layer.screen_y = screen_y;
            layer.canvas = Canvas::new(rect_size.height, rect_size.width);

            if let Some((surface_y, surface_x)) = layer.surface.cursor_position(ctx) {
                prev_cursor_pos = Some((screen_y + surface_y, screen_x + surface_x));
            }
        }

        let prev_screen_index = self.active_screen_index;
        self.active_screen_index = (self.active_screen_index + 1) % self.screens.len();
        let screen_index = self.active_screen_index;

        // Render and composite layers.
        compose_layers(ctx, &mut self.screens[screen_index], self.layers.iter_mut());

        // Get the cursor position.
        let mut cursor = None;
        for layer in self.layers.iter().rev() {
            if layer.surface.is_active(ctx) {
                if let Some((y, x)) = layer.surface.cursor_position(ctx) {
                    cursor = Some((layer.screen_y + y, layer.screen_x + x));
                    break;
                }
            }
        }

        // Compute diffs.
        let draw_ops =
            self.screens[screen_index].compute_draw_updates(&self.screens[prev_screen_index]);

        self.renderer_tx.send((draw_ops, cursor)).oops();
    }

    pub fn handle_input(&mut self, ctx: &mut C, input: InputEvent) {
        trace!("input: {:?}", input);
        match input {
            InputEvent::Key(key) => {
                self.past_layers = Vec::new();
                while let Some(mut layer) = self.layers.pop() {
                    let result = if layer.surface.is_active(ctx) {
                        layer.surface.handle_key_event(self, ctx, key)
                    } else {
                        HandledEvent::Ignored
                    };
                    self.past_layers.push(layer);
                    if result == HandledEvent::Consumed {
                        break;
                    }
                }
                self.layers.extend(self.past_layers.drain(..).rev());
            }
            InputEvent::Mouse(ev) => {
                self.past_layers = Vec::new();
                while let Some(mut layer) = self.layers.pop() {
                    let screen_y = ev.row as usize;
                    let screen_x = ev.column as usize;
                    let in_bounds = layer.screen_y <= screen_y
                        && screen_y < layer.screen_y + layer.canvas.height()
                        && layer.screen_x <= screen_x
                        && screen_x < layer.screen_x + layer.canvas.width();

                    let result = if layer.surface.is_active(ctx) && in_bounds {
                        layer.surface.handle_mouse_event(
                            self,
                            ctx,
                            ev.kind,
                            ev.modifiers,
                            screen_y - layer.screen_y,
                            screen_x - layer.screen_x,
                        )
                    } else {
                        HandledEvent::Ignored
                    };

                    self.past_layers.push(layer);
                    if result == HandledEvent::Consumed {
                        break;
                    }
                }
                self.layers.extend(self.past_layers.drain(..).rev());
            }
            InputEvent::KeyBatch(input) => {
                self.past_layers = Vec::new();
                while let Some(mut layer) = self.layers.pop() {
                    let result = if layer.surface.is_active(ctx) {
                        layer.surface.handle_key_batch_event(self, ctx, &input)
                    } else {
                        HandledEvent::Ignored
                    };

                    self.past_layers.push(layer);
                    if result == HandledEvent::Consumed {
                        break;
                    }
                }
                self.layers.extend(self.past_layers.drain(..).rev());
            }
        }
    }
}

/// Renders each surfaces and copy the compose into the screen canvas.
fn compose_layers<C>(ctx: &mut C, screen: &mut Canvas, layers: slice::IterMut<'_, Layer<C>>) {
    screen.view_mut().clear();

    for layer in layers {
        if !layer.surface.is_active(ctx) {
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

        layer.surface.render(ctx, &mut layer.canvas.view_mut());
        screen.copy_from_other(layer.screen_y, layer.screen_x, &layer.canvas);
    }
}

fn relayout_layer<C>(
    ctx: &mut C,
    surface: &mut (impl Surface<Context = C> + ?Sized),
    screen_size: RectSize,
    prev_cursor_pos: Option<(usize, usize)>,
) -> ((usize, usize), RectSize) {
    let (layout, rect) = surface.layout(ctx, screen_size);

    let (screen_y, screen_x) = match layout {
        Layout::Fixed { y, x } => (y, x),
        Layout::Center => (
            (screen_size.height / 2).saturating_sub(rect.height / 2),
            (screen_size.width / 2).saturating_sub(rect.width / 2),
        ),
        Layout::AroundCursor => {
            let (cursor_y, cursor_x) = prev_cursor_pos.unwrap();
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
