use std::slice;

use crossterm::event::MouseEvent;
use noa_common::time_report::TimeReport;
use tokio::sync::mpsc;

use crate::{surface::HandledEvent, terminal::InputEvent};

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
    screens: [Canvas; 2],
    screen_size: RectSize,
    active_screen_index: usize,
    /// The last element comes foreground.
    layers: Vec<Layer<C>>,
}

#[allow(clippy::new_without_default)]
impl<C> Compositor<C> {
    pub fn new() -> Compositor<C> {
        let (term_tx, term_rx) = mpsc::unbounded_channel();
        let terminal = Terminal::new(move |ev| {
            term_tx.send(ev).ok();
        });

        let screen_size = RectSize {
            height: terminal.height(),
            width: terminal.width(),
        };

        Compositor {
            terminal,
            term_rx,
            screens: [
                Canvas::new(screen_size.height, screen_size.width),
                Canvas::new(screen_size.height, screen_size.width),
            ],
            screen_size,
            active_screen_index: 0,
            layers: Vec::new(),
        }
    }

    pub async fn recv_terminal_event(&mut self) -> Option<terminal::Event> {
        self.term_rx.recv().await
    }

    pub fn add_frontmost_layer(&mut self, surface: Box<dyn Surface<Context = C> + Send>) {
        self.layers.push(Layer {
            surface,
            canvas: Canvas::new(0, 0),
            screen_x: 0,
            screen_y: 0,
        });
    }

    pub fn resize_screen(&mut self, height: usize, width: usize) {
        self.screen_size = RectSize { height, width };
        self.screens = [Canvas::new(height, width), Canvas::new(height, width)];
        self.terminal.clear();
    }

    pub fn render_to_terminal(&mut self, ctx: &mut C) {
        let rendering_time = TimeReport::new("rendering time");

        // Re-layout layers.
        for layer in self.layers.iter_mut() {
            let ((screen_y, screen_x), rect_size) =
                relayout_layers(ctx, &*layer.surface, self.screen_size);
            layer.screen_x = screen_x;
            layer.screen_y = screen_y;
            layer.canvas = Canvas::new(rect_size.height, rect_size.width);
        }

        let prev_screen_index = self.active_screen_index;
        self.active_screen_index = (self.active_screen_index + 1) % self.screens.len();
        let screen_index = self.active_screen_index;

        // Render and composite layers.
        compose_layers(ctx, &mut self.screens[screen_index], self.layers.iter_mut());

        // Get the cursor position.
        let mut cursor = None;
        for layer in self.layers.iter().rev() {
            if let Some((y, x)) = layer.surface.cursor_position(ctx) {
                cursor = Some((layer.screen_y + y, layer.screen_x + x));
                break;
            }
        }

        // Compute diffs.
        let draw_ops =
            self.screens[screen_index].compute_draw_updates(&self.screens[prev_screen_index]);

        // Write into stdout.
        drop(rendering_time);
        let _drawer_time = TimeReport::new("drawer time");
        trace!("draw changes: {} items", draw_ops.len());
        let mut drawer = self.terminal.drawer();
        for op in draw_ops {
            drawer.draw(&op);
        }
        if let Some((screen_y, screen_x)) = cursor {
            drawer.show_cursor(screen_y, screen_x);
        }

        drawer.flush();
    }

    pub fn handle_input(&mut self, ctx: &mut C, input: InputEvent) {
        trace!("input: {:?}", input);
        match input {
            InputEvent::Key(key) => {
                for layer in self.layers.iter_mut().rev() {
                    if let HandledEvent::Consumed = layer.surface.handle_key_event(ctx, key) {
                        break;
                    }
                }
            }
            InputEvent::Mouse(ev) => {
                for layer in self.layers.iter_mut().rev() {
                    let screen_y = ev.row as usize;
                    let screen_x = ev.column as usize;
                    if layer.screen_y <= screen_y
                        && screen_y < layer.screen_y + layer.canvas.height()
                        && layer.screen_x <= screen_x
                        && screen_x < layer.screen_x + layer.canvas.width()
                    {
                        if let HandledEvent::Consumed = layer.surface.handle_mouse_event(
                            ctx,
                            ev.kind,
                            ev.modifiers,
                            screen_y - layer.screen_y,
                            screen_x - layer.screen_x,
                        ) {
                            break;
                        }
                    }
                }
            }
            InputEvent::KeyBatch(input) => {
                for layer in self.layers.iter_mut().rev() {
                    if let HandledEvent::Consumed =
                        layer.surface.handle_key_batch_event(ctx, &input)
                    {
                        break;
                    }
                }
            }
        }
    }
}

/// Renders each surfaces and copy the compose into the screen canvas.
fn compose_layers<C>(ctx: &mut C, screen: &mut Canvas, layers: slice::IterMut<'_, Layer<C>>) {
    screen.view_mut().clear();

    for layer in layers {
        if !layer.surface.is_visible(ctx) {
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

fn relayout_layers<C>(
    ctx: &mut C,
    surface: &(impl Surface<Context = C> + ?Sized),
    screen_size: RectSize,
) -> ((usize, usize), RectSize) {
    let (layout, rect) = surface.layout(ctx, screen_size);

    let (screen_y, screen_x) = match layout {
        Layout::Fixed { y, x } => (y, x),
        Layout::Center => (
            (screen_size.height / 2).saturating_sub(rect.height / 2),
            (screen_size.width / 2).saturating_sub(rect.width / 2),
        ),
        Layout::AroundCursor => {
            let (cursor_y, cursor_x) = surface.cursor_position(ctx).unwrap();

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
