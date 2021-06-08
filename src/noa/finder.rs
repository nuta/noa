use std::{
    cmp::{max, min},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;

use crate::ui::{Canvas, Compositor, Context, Event, Layout, RectSize, Surface};

enum Item {
    File(PathBuf),
}

pub struct FinderSurface {
    input: String,
    items: Arc<Mutex<Vec<Item>>>,
}

impl FinderSurface {
    pub fn new(ctx: &mut Context) -> FinderSurface {
        let items = Arc::new(Mutex::new(Vec::new()));
        tokio::spawn(update_items(ctx.event_tx.clone(), items.clone()));
        FinderSurface {
            input: String::new(),
            items,
        }
    }
}

impl Surface for FinderSurface {
    fn name(&self) -> &str {
        "finder"
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        let rect_size = RectSize {
            width: min(max(screen_size.width, 32), 80),
            height: min(max(screen_size.height, 8), 16),
        };
        (Layout::Center, rect_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        // TODO:
        None
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        self.render_all(ctx, canvas)
    }

    fn render_all(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        // TODO:
    }

    fn handle_key_event(&mut self, ctx: &mut Context, compositor: &mut Compositor, key: KeyEvent) {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let updated = match (key.modifiers, key.code) {
            (NONE, KeyCode::Enter) => false,
            _ => {
                return;
            }
        };

        tokio::spawn(update_items(ctx.event_tx.clone(), self.items.clone()));
    }

    fn handle_key_batch_event(
        &mut self,
        ctx: &mut Context,
        _compositor: &mut Compositor,
        input: &str,
    ) {
        // TODO:
    }
}

async fn update_items(event_tx: UnboundedSender<Event>, items: Arc<Mutex<Vec<Item>>>) {}
