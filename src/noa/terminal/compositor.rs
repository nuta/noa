use std::{collections::HashMap, sync::Arc};

use crossterm::event::KeyEvent;
use parking_lot::Mutex;

use crate::{
    editor::Editor,
    surfaces::{Context, Surface},
};

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
    active: bool,
    canvas: Canvas,
    screen_y: usize,
    screen_x: usize,
}

pub struct Compositor {
    terminal: Terminal,
    screens: [Canvas; 2],
    active_screen_index: usize,
    layers: Vec<Layer>,
}

impl Compositor {
    pub fn new(terminal: Terminal) -> Compositor {
        let layers = vec![];
        let screen = [
            Canvas::new(terminal.height(), terminal.width()),
            Canvas::new(terminal.height(), terminal.width()),
        ];
        Compositor {
            terminal,
            layers,
            screens: screen,
            active_screen_index: 0,
        }
    }

    pub fn render(&mut self, editor: &mut Editor) {
        let prev_screen_index = self.active_screen_index;
        self.active_screen_index = (self.active_screen_index + 1) & self.screens.len();
        let next_screen = &mut self.screens[self.active_screen_index];

        // Render each surfaces and copy the compose into the screen canvas.
        let mut ctx = Context { editor };
        for layer in self.layers.iter_mut().rev() {
            if !layer.active || !layer.surface.invalidated(&mut ctx) {
                continue;
            }

            layer.surface.render(&mut ctx, &mut layer.canvas);
            next_screen.copy_from_other(layer.screen_y, layer.screen_x, &layer.canvas);
        }

        let next_screen = &self.screens[self.active_screen_index];
        let prev_screen = &self.screens[prev_screen_index];
        if let Some(mut drawer) = self.terminal.drawer() {
            for op in next_screen.compute_draw_updates(&prev_screen) {
                drawer.draw(&op);
            }
        }
    }

    pub fn handle_event(&mut self, editor: &mut Editor, ev: Event) {
        let mut ctx = Context { editor };
        let result = match ev {
            Event::Key(key) => self
                .active_layer_mut()
                .surface
                .handle_key_event(&mut ctx, key),
            Event::KeyBatch(input) => self
                .active_layer_mut()
                .surface
                .handle_key_batch_event(&mut ctx, &input),
            _ => {
                trace!("unhandled event = {:?}", ev);
                Ok(())
            }
        };

        if let Err(err) = result {
            error!("surface returned an error: {}", err);
        }
    }

    pub fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers
            .iter()
            .find(|layer| layer.surface.name() == name)
    }

    pub fn layer_mut(&mut self, name: &str) -> Option<&mut Layer> {
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

        unreachable!("at least the buffer surface is always active");
    }
}
