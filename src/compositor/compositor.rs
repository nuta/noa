use std::slice;

use super::{
    canvas::Canvas,
    surface::{Layout, RectSize, Surface},
    terminal::Terminal,
};

pub struct Layer {
    pub surface: Box<dyn Surface + Send>,
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
    layers: Vec<Layer>,
}

impl Compositor {
    pub fn new(terminal: Terminal) -> Compositor {
        let screen_size = RectSize {
            height: terminal.height(),
            width: terminal.width(),
        };

        Compositor {
            terminal,
            screens: [
                Canvas::new(screen_size.height, screen_size.width),
                Canvas::new(screen_size.height, screen_size.width),
            ],
            screen_size,
            active_screen_index: 0,
            layers: Vec::new(),
        }
    }

    pub fn resize_screen(&mut self, height: usize, width: usize) {
        self.screen_size = RectSize { height, width };
        self.screens = [Canvas::new(height, width), Canvas::new(height, width)];
        self.terminal.clear();
    }

    pub fn render_to_terminal(&mut self, cursor_pos: (usize, usize)) {
        // Re-layout layers.
        for layer in &mut self.layers {
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
        compose_layers(&mut self.screens[screen_index], self.layers.iter_mut());

        // Get the cursor position.
        let mut cursor = None;
        for layer in self.layers.iter().rev() {
            if layer.active {
                if let Some((y, x)) = layer.surface.cursor_position() {
                    cursor = Some((layer.screen_y + y, layer.screen_x + x));
                    break;
                }
            }
        }

        // Compute diffs.

        let draw_ops =
            self.screens[screen_index].compute_draw_updates(&self.screens[prev_screen_index]);

        // Write into stdout.
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
}

/// Renders each surfaces and copy the compose into the screen canvas.
fn compose_layers(screen: &mut Canvas, layers: slice::IterMut<'_, Layer>) {
    screen.view_mut().clear();

    for layer in layers {
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
