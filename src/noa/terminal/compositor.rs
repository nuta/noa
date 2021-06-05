use std::{slice, time::Instant};

use crossterm::event::KeyEvent;
use noa_common::warn_on_error;

use crate::surfaces::{buffer::BufferSurface, too_small::TooSmallSurface, Context, Surface};

use super::{canvas::Canvas, Terminal};

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
        let screen = [
            Canvas::new(terminal.height(), terminal.width()),
            Canvas::new(terminal.height(), terminal.width()),
        ];

        let layers = create_layers(terminal.height(), terminal.width());

        Compositor {
            terminal,
            layers,
            screens: screen,
            active_screen_index: 0,
        }
    }

    pub fn resize_screen(&mut self, ctx: &mut Context, height: usize, width: usize) {
        self.layers = create_layers(height, width);

        self.screens = [Canvas::new(height, width), Canvas::new(height, width)];
        let active_screen = &mut self.screens[self.active_screen_index];
        compose_layers(ctx, active_screen, self.layers.iter_mut(), true);
    }

    pub fn render_to_terminal(&mut self, ctx: &mut Context) {
        let prev_screen_index = self.active_screen_index;
        self.active_screen_index = (self.active_screen_index + 1) & self.screens.len();
        let next_screen = &mut self.screens[self.active_screen_index];

        compose_layers(ctx, next_screen, self.layers.iter_mut(), false);

        let next_screen = &self.screens[self.active_screen_index];
        let prev_screen = &self.screens[prev_screen_index];
        let draw_ops = next_screen.compute_draw_updates(&prev_screen);

        trace!("draw changes: {} items", draw_ops.len());
        let started_at = Instant::now();
        let mut drawer = self.terminal.drawer();
        for op in draw_ops {
            drawer.draw(&op);
        }

        drawer.flush();
        trace!("draw took {:?}", started_at.elapsed());
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
                trace!("unhandled event = {:?}", ev);
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

fn create_layers(screen_height: usize, screen_width: usize) -> Vec<Layer> {
    if screen_width < 10 || screen_height < 5 {
        // The screen is too small.
        return vec![Layer {
            surface: Box::new(TooSmallSurface::new("too small!")),
            visible: true,
            active: true,
            canvas: Canvas::new(screen_height, screen_width),
            screen_x: 0,
            screen_y: 0,
        }];
    }

    let buffer_height = screen_height - 2;
    let buffer_width = screen_width;
    vec![Layer {
        surface: Box::new(BufferSurface::new()),
        visible: true,
        active: true,
        canvas: Canvas::new(buffer_height, buffer_width),
        screen_x: 0,
        screen_y: 0,
    }]
}
