use std::{collections::HashMap, sync::Arc};

use crossterm::event::KeyEvent;
use parking_lot::Mutex;

use crate::surfaces::Surface;

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

pub struct Compositor {
    surfaces: HashMap<String, Arc<Mutex<dyn Surface>>>,
}

impl Compositor {
    pub fn new() -> Compositor {
        Compositor {
            surfaces: HashMap::new(),
        }
    }

    pub fn handle_event(&mut self, ev: Event) {
        match ev {
            Event::Key(key) => {}
            Event::KeyBatch(str) => {}
            _ => {
                trace!("unhandled event = {:?}", ev);
            }
        }
    }
}
