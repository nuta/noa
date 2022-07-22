use noa_buffer::reflow_iter::{PrintableGrapheme, ReflowItem};
use noa_compositor::{
    canvas::{CanvasViewMut, Grapheme},
    compositor::Compositor,
    surface::{HandledEvent, KeyEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers},
};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::editor::Editor;

enum MainloopCommand {
    Quit,
}

pub struct Ui {
    compositor: Compositor<Editor>,
    editor: Editor,
}

impl Ui {
    pub fn new(editor: Editor) -> Self {
        Ui {
            compositor: Compositor::new(),
            editor,
        }
    }

    pub async fn run(mut self) {
        let (mainloop_tx, mut mainloop_rx) = mpsc::unbounded_channel();
        self.compositor
            .add_frontmost_layer(Box::new(Text::new(mainloop_tx.clone())));
        loop {
            self.compositor.render(&mut self.editor);
            tokio::select! {
                biased;

                Some(command) = mainloop_rx.recv() => {
                    match command {
                        MainloopCommand::Quit => break,
                    }
                }

                _ = self.compositor.handle_event(&mut self.editor) => {
                }
            }
        }
    }
}

struct Text {
    mainloop_tx: UnboundedSender<MainloopCommand>,
}

impl Text {
    pub fn new(mainloop_tx: UnboundedSender<MainloopCommand>) -> Self {
        Text { mainloop_tx }
    }
}

impl Surface for Text {
    type Context = Editor;

    fn name(&self) -> &str {
        "buffer"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, ctx: &mut Self::Context) -> bool {
        true
    }

    fn layout(
        &mut self,
        ctx: &mut Self::Context,
        screen_size: noa_compositor::surface::RectSize,
    ) -> (
        noa_compositor::surface::Layout,
        noa_compositor::surface::RectSize,
    ) {
        (
            Layout::Fixed { y: 0, x: 0 },
            RectSize {
                width: screen_size.width,
                height: screen_size.height.saturating_sub(2),
            },
        )
    }

    fn cursor_position(&self, ctx: &mut Self::Context) -> Option<(usize, usize)> {
        None
    }

    fn handle_key_event(
        &mut self,
        editor: &mut Editor,
        _compositor: &mut Compositor<Self::Context>,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.current_document_mut();

        let mut show_completion = false;
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.mainloop_tx.send(MainloopCommand::Quit);
            }
            (KeyCode::Esc, NONE) => {
                doc.clear_secondary_cursors();
            }
            _ => {}
        }

        HandledEvent::Consumed
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        let doc = editor.current_document();
        for ReflowItem {
            grapheme,
            grapheme_width,
            pos_in_screen,
            pos_in_buffer,
        } in doc.reflow_iter(doc.top_left, canvas.width(), doc.editorconfig().tab_width)
        {
            if pos_in_screen.y >= canvas.height() {
                break;
            }

            match grapheme {
                PrintableGrapheme::Grapheme(grapheme) => {
                    canvas.write(
                        pos_in_screen.y,
                        pos_in_screen.x,
                        Grapheme::new_with_width(grapheme, grapheme_width),
                    );
                }
                PrintableGrapheme::Whitespaces
                | PrintableGrapheme::ZeroWidth
                | PrintableGrapheme::EndOfLine => {
                    // Already filled with whitespaces by `canvas.clear()`.
                }
            }
        }
    }
}
