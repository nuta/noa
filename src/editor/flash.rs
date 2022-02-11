use std::time::{Duration, Instant};

use noa_buffer::cursor::Range;

use crate::view::View;

const FLASH_DURATION_MS: Duration = Duration::from_millis(360);

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

    pub fn next_timeout(&self) -> Option<Duration> {
        if self.flashes.is_empty() {
            None
        } else {
            Some(Duration::from_millis(20))
        }
    }

    pub fn highlight(&mut self, view: &mut View) {
        self.flashes.retain_mut(|flash| match flash.flashed_at {
            Some(flashed_at) if flashed_at.elapsed() > FLASH_DURATION_MS => false,
            Some(flashed_at) => {
                let total = FLASH_DURATION_MS.as_millis();
                let elapsed = flashed_at.elapsed().as_millis();
                if total / 3 <= elapsed && elapsed < total * 2 / 3 {
                    view.clear_highlight(flash.range);
                } else {
                    view.highlight(flash.range, "flash");
                }
                true
            }
            None => {
                flash.flashed_at = Some(Instant::now());
                view.highlight(flash.range, "flash");
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
