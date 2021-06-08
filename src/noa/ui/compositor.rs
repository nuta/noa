use std::{slice, time::Instant};

use crossterm::event::KeyEvent;
use noa_common::{time_report::TimeReport, warn_on_error};

use crate::ui::{
    surfaces::{buffer::BufferSurface, too_small::TooSmallSurface},
    Context, Surface,
};

use crate::terminal::Terminal;

use super::{Canvas, Layout, RectSize};

#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    KeyBatch(String),
    NoCompletion,
    Resize {
        screen_height: usize,
        screen_width: usize,
    },
}

pub struct Layer {
    surface: Box<dyn Surface>,
    /// If it's `false`, the surface won't receive key events.
    active: bool,
    visible: bool,
    canvas: Canvas,
    layout: Layout,
    screen_y: usize,
    screen_x: usize,
}

pub struct Compositor {
    terminal: Terminal,
    screens: [Canvas; 2],
    active_screen_index: usize,
    /// The last element comes foreground.
    layers: Vec<Layer>,
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

        let layers = create_layers(screen_size);

        Compositor {
            terminal,
            layers,
            screens: screen,
            active_screen_index: 0,
        }
    }

    pub fn resize_screen(&mut self, ctx: &mut Context, height: usize, width: usize) {
        let screen_size = RectSize { height, width };
        self.layers = create_layers(screen_size);

        self.screens = [Canvas::new(height, width), Canvas::new(height, width)];
        let active_screen = &mut self.screens[self.active_screen_index];
        compose_layers(ctx, active_screen, self.layers.iter_mut(), true);
    }

    pub fn render_to_terminal(&mut self, ctx: &mut Context) {
        let prev_screen_index = self.active_screen_index;
        self.active_screen_index = (self.active_screen_index + 1) % self.screens.len();
        let screen_index = self.active_screen_index;

        // Render and composite layers.
        let compose_layers_time = TimeReport::new("compose_layers");
        compose_layers(
            ctx,
            &mut self.screens[screen_index],
            self.layers.iter_mut(),
            false,
        );
        compose_layers_time.report();

        // Get the cursor position.
        let active_layer = self.active_layer();
        let cursor = active_layer
            .surface
            .cursor_position()
            .map(|(y, x)| (active_layer.screen_y + y, active_layer.screen_x + x));

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
        let result = match ev {
            Event::Key(key) => self.active_layer_mut().surface.handle_key_event(ctx, key),
            Event::KeyBatch(input) => self
                .active_layer_mut()
                .surface
                .handle_key_batch_event(ctx, &input),
            Event::Resize {
                screen_height,
                screen_width,
            } => {
                self.resize_screen(ctx, screen_height, screen_width);
                Ok(())
            }
            _ => {
                trace!("unhandled event: {:?}", ev);
                Ok(())
            }
        };

        if let Err(err) = result {
            error!("surface returned an error: {}", err);
        }
    }

    pub fn _layer(&self, name: &str) -> Option<&Layer> {
        self.layers
            .iter()
            .find(|layer| layer.surface.name() == name)
    }

    pub fn _layer_mut(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers
            .iter_mut()
            .find(|layer| layer.surface.name() == name)
    }

    pub fn active_layer(&self) -> &Layer {
        for layer in self.layers.iter().rev() {
            if layer.active {
                return layer;
            }
        }

        unreachable!("at least buffer or too_small surface is always active");
    }

    pub fn active_layer_mut(&mut self) -> &mut Layer {
        for layer in self.layers.iter_mut().rev() {
            if layer.active {
                return layer;
            }
        }

        unreachable!("at least buffer or too_small surface is always active");
    }
}

/// Renders each surfaces and copy the compose into the screen canvas.
fn compose_layers<'a, 'b, 'c>(
    ctx: &'a mut Context,
    screen: &'b mut Canvas,
    layers: slice::IterMut<'c, Layer>,
    render_all: bool,
) {
    screen.clear();

    for layer in layers {
        if !layer.visible {
            continue;
        }

        if render_all {
            warn_on_error!(
                layer.surface.render_all(ctx, &mut layer.canvas),
                "Surface::render_all() returned an error"
            );
        } else {
            warn_on_error!(
                layer.surface.render(ctx, &mut layer.canvas),
                "Surface::render() returned an error"
            );
        }

        screen.copy_from_other(layer.screen_y, layer.screen_x, &layer.canvas);
    }
}

fn create_layers(screen_size: RectSize) -> Vec<Layer> {
    let mut layers = Vec::with_capacity(16);

    if screen_size.width < 10 || screen_size.height < 5 {
        // The screen is too small.
        push_layer(&mut layers, screen_size, TooSmallSurface::new("too small!"));
        return layers;
    }

    push_layer(&mut layers, screen_size, BufferSurface::new());

    layers
}

fn push_layer(layers: &mut Vec<Layer>, screen_size: RectSize, surface: impl Surface + 'static) {
    let (layout, rect_size) = surface.layout(screen_size);
    layers.push(Layer {
        surface: Box::new(surface),
        visible: true,
        active: true,
        canvas: Canvas::new(rect_size.height, rect_size.width),
        layout,
        screen_x: 0,
        screen_y: 0,
    });
}
