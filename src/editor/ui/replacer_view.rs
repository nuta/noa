use std::path::PathBuf;

use noa_compositor::{
    canvas::CanvasViewMut,
    line_edit::LineEdit,
    surface::{HandledEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind},
    Compositor,
};
use tokio::sync::mpsc;

use crate::{
    config::theme_for,
    editor::Editor,
    job::JobManager,
    search::{search_paths_globally, CancelFlag},
};

enum ReplacerMode {
    Files,
    Buffers,
}

enum Focus {
    SearchInput,
    ReplaceInput,
}

enum ReplacerItem {
    File { path: String },
}

pub struct ReplacerView {
    workspace_dir: PathBuf,
    mode: ReplacerMode,
    focus: Focus,
    active: bool,
    scroll: usize,
    items: Vec<ReplacerItem>,
    search_input: LineEdit,
    replace_input: LineEdit,
    cancel_flag: Option<CancelFlag>,
}

impl ReplacerView {
    pub fn new(workspace_dir: PathBuf) -> ReplacerView {
        ReplacerView {
            workspace_dir,
            mode: ReplacerMode::Buffers,
            focus: Focus::SearchInput,
            active: false,
            scroll: 0,
            items: Vec::new(),
            search_input: LineEdit::new(),
            replace_input: LineEdit::new(),
            cancel_flag: None,
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.focus = Focus::SearchInput;
        self.scroll = 0;
        self.search_input.clear();
        self.replace_input.clear();
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn update(&mut self, jobs: &mut JobManager) {
        match self.mode {
            ReplacerMode::Files => self.search_paths(jobs),
            ReplacerMode::Buffers => self.search_texts(),
        }
    }

    pub fn search_paths(&mut self, jobs: &mut JobManager) {
        let cancel_flag = CancelFlag::new();
        if let Some(prev_cancel_flag) = self.cancel_flag.replace(cancel_flag.clone()) {
            prev_cancel_flag.cancel();
        }

        let (items_tx, mut items_rx) = mpsc::unbounded_channel();
        let workspace_dir = self.workspace_dir.clone();
        let query = self.search_input.text();
        tokio::task::spawn_blocking(move || {
            if let Err(err) =
                search_paths_globally(&workspace_dir, &query, items_tx, None, cancel_flag.clone())
            {
                notify_warn!("failed to search path: {}", err);
            }
        });

        jobs.await_in_mainloop(
            async move {
                let mut items = Vec::new();
                while let Some((_, path)) = items_rx.recv().await {
                    items.push(ReplacerItem::File { path });
                }
                items
            },
            |_, compositor, items| {
                let replacer = compositor.get_mut_surface_by_name::<ReplacerView>("replacer");
                replacer.items = items;
            },
        );
    }

    pub fn search_texts(&mut self) {}
}

impl Surface for ReplacerView {
    type Context = Editor;

    fn name(&self) -> &str {
        "replacer"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _editor: &mut Editor) -> bool {
        self.active
    }

    fn layout(&mut self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        (
            Layout::Fixed { y: 0, x: 0 },
            RectSize {
                width: screen_size.width,
                height: screen_size
                    .height
                    .saturating_sub(2 /* meta line's height */),
            },
        )
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, _editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        let mode_text = match self.mode {
            ReplacerMode::Files => "Rename Files",
            ReplacerMode::Buffers => "Replace Texts",
        };

        canvas.write_str(0, 1, mode_text);
        canvas.write_str_with_style(1, 0, "  Search  ", theme_for("label"));
        canvas.write_str_with_style(2, 0, " New Text ", theme_for("label"));
        canvas.write_str(1, 11, &self.search_input.text());
        canvas.write_str(2, 11, &self.replace_input.text());
    }

    fn handle_key_event(
        &mut self,
        _editor: &mut Editor,
        _compositor: &mut Compositor<Self::Context>,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.close();
            }
            (KeyCode::Tab, NONE) => {
                self.focus = match self.focus {
                    Focus::SearchInput => Focus::ReplaceInput,
                    Focus::ReplaceInput => Focus::SearchInput,
                };
            }
            // Toggle mode.
            (KeyCode::F(1), NONE) => {
                self.mode = match self.mode {
                    ReplacerMode::Files => ReplacerMode::Buffers,
                    ReplacerMode::Buffers => ReplacerMode::Files,
                };
            }
            _ => {
                return match self.focus {
                    Focus::SearchInput => self.search_input.consume_key_event(key),
                    Focus::ReplaceInput => self.replace_input.consume_key_event(key),
                };
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut Self::Context,
        _compositor: &mut Compositor<Editor>,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }

    fn handle_mouse_event(
        &mut self,
        _ctx: &mut Self::Context,
        _compositor: &mut Compositor<Self::Context>,
        _kind: MouseEventKind,
        _modifiers: KeyModifiers,
        _surface_y: usize,
        _surface_x: usize,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }
}
