use std::time::{Duration, Instant};

use noa_buffer::cursor::Range;

use crate::{theme::ThemeKey, view::View};

const FLASH_DURATION_MS: Duration = Duration::from_millis(500);

pub struct Flash {
    range: Range,
    flashed_at: Option<Instant>,
}

pub struct FlashManager {
    flashes: Vec<Flash>,
}

impl FlashManager {
    pub fn new() -> FlashManager {
        FlashManager {
            flashes: Vec::new(),
        }
    }

    pub fn highlight(&mut self, view: &mut View) {
        self.flashes.retain_mut(|flash| match flash.flashed_at {
            Some(flashed_at) if flashed_at.elapsed() > FLASH_DURATION_MS => false,
            Some(_) => {
                view.highlight(flash.range, ThemeKey::Flash);
                true
            }
            None => {
                flash.flashed_at = Some(Instant::now());
                view.highlight(flash.range, ThemeKey::Flash);
                true
            }
        });
    }

    pub fn flash(&mut self, range: Range) {
        self.flashes.push(Flash {
            range,
            flashed_at: None,
        });
    }
}
