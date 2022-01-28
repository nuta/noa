use std::path::Path;

use noa_compositor::{
    canvas::CanvasViewMut,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::KeyEvent,
    Compositor,
};

use crate::{editor::Editor, path::PathFinder};

pub struct FinderView {
    path_finder: PathFinder,
}

impl FinderView {
    pub fn new(workspace_dir: &Path) -> FinderView {
        FinderView {
            path_finder: PathFinder::new(workspace_dir),
        }
    }
}

impl Surface for FinderView {
    type Context = Editor;

    fn name(&self) -> &str {
        "finder"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_visible(&self, _editor: &mut Editor) -> bool {
        false
    }

    fn layout(&self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Fixed { x: 0, y: 0 }, screen_size)
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, _editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {}

    fn handle_key_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        _editor: &mut Editor,
        _key: KeyEvent,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor<Editor>,
        _editor: &mut Editor,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }
}
