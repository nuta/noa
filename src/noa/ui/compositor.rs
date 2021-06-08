use std::{slice, sync::Arc, time::Instant};

use anyhow::Result;
use crossterm::event::KeyEvent;
use noa_common::{time_report::TimeReport, warn_on_error};
use parking_lot::Mutex;

use crate::ui::{Context, Surface};

use crate::terminal::Terminal;

use super::{truncate_to_width, Canvas, Layout, RectSize};

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
    layers: Vec<Arc<Mutex<Layer>>>,
    layer_ctor: Box<dyn Fn(RectSize) -> Vec<Box<dyn Surface>>>,
}

impl Compositor {
    pub fn new<F>(terminal: Terminal, layer_ctor: F) -> Compositor
    where
        F: Fn(RectSize) -> Vec<Box<dyn Surface>> + 'static,
    {
        let screen_size = RectSize {
            height: terminal.height(),
            width: terminal.width(),
        };

        let screen = [
            Canvas::new(screen_size.height, screen_size.width),
            Canvas::new(screen_size.height, screen_size.width),
        ];

        let layer_ctor: Box<dyn Fn(RectSize) -> Vec<Box<dyn Surface>>> = Box::new(layer_ctor);
        let layers = create_layers(&layer_ctor, screen_size);

        Compositor {
            terminal,
            layers,
            screens: screen,
            active_screen_index: 0,
            layer_ctor,
        }
    }

    pub fn resize_screen(&mut self, ctx: &mut Context, height: usize, width: usize) {
        let screen_size = RectSize { height, width };
        self.layers = create_layers(&self.layer_ctor, screen_size);

        self.screens = [Canvas::new(height, width), Canvas::new(height, width)];
        let active_screen = &mut self.screens[self.active_screen_index];
        compose_layers(ctx, active_screen, self.layers.iter(), true);
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
            self.layers.iter(),
            false,
        );
        compose_layers_time.report();

        // Get the cursor position.
        let cursor = {
            let active_layer = self.active_layer().lock();
            active_layer
                .surface
                .cursor_position()
                .map(|(y, x)| (active_layer.screen_y + y, active_layer.screen_x + x))
        };

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
            Event::Key(key) => self
                .active_layer()
                .clone()
                .lock()
                .surface
                .handle_key_event(ctx, self, key),
            Event::KeyBatch(input) => self
                .active_layer()
                .clone()
                .lock()
                .surface
                .handle_key_batch_event(ctx, self, &input),
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

    pub fn active_layer(&self) -> &Arc<Mutex<Layer>> {
        for layer_lock in self.layers.iter().rev() {
            let layer = layer_lock.lock();
            if layer.active {
                return layer_lock;
            }
        }

        unreachable!("at least buffer or too_small surface is always active");
    }
}

/// Renders each surfaces and copy the compose into the screen canvas.
fn compose_layers<'a, 'b, 'c>(
    ctx: &'a mut Context,
    screen: &'b mut Canvas,
    layers: slice::Iter<'c, Arc<Mutex<Layer>>>,
    render_all: bool,
) {
    screen.clear();

    for layer in layers {
        let mut layer = layer.lock();
        let layer = &mut *layer;

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

fn create_layers(
    ctor: &Box<dyn Fn(RectSize) -> Vec<Box<dyn Surface>>>,
    screen_size: RectSize,
) -> Vec<Arc<Mutex<Layer>>> {
    let mut layers = Vec::with_capacity(16);

    if screen_size.width < 10 || screen_size.height < 5 {
        // The screen is too small.
        push_layer(
            &mut layers,
            screen_size,
            Box::new(TooSmallSurface::new("too small!")),
        );
        return layers;
    }

    for surface in ctor(screen_size) {
        push_layer(&mut layers, screen_size, surface);
    }
    layers
}

fn push_layer(
    layers: &mut Vec<Arc<Mutex<Layer>>>,
    screen_size: RectSize,
    surface: Box<dyn Surface>,
) {
    let (layout, rect_size) = surface.layout(screen_size);

    let (screen_y, screen_x) = match layout {
        Layout::Center => (
            (screen_size.height / 2).saturating_sub(rect_size.height / 2),
            (screen_size.width / 2).saturating_sub(rect_size.width / 2),
        ),
        Layout::Full => (0, 0),
    };

    layers.push(Arc::new(Mutex::new(Layer {
        surface,
        visible: true,
        active: true,
        canvas: Canvas::new(rect_size.height, rect_size.width),
        layout,
        screen_x,
        screen_y,
    })));
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

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Full, screen_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()> {
        self.render_all(ctx, canvas)
    }

    fn render_all(&mut self, _ctx: &mut Context, canvas: &mut Canvas) -> Result<()> {
        canvas.set_str(0, 0, truncate_to_width(&self.text, canvas.width()));
        Ok(())
    }

    fn handle_key_event(
        &mut self,
        _ctx: &mut Context,
        _compositor: &mut Compositor,
        _key: KeyEvent,
    ) -> Result<()> {
        Ok(())
    }

    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut Context,
        _compositor: &mut Compositor,
        _input: &str,
    ) -> Result<()> {
        Ok(())
    }
}
