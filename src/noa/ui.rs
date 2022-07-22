use noa_buffer::reflow_iter::{PrintableGrapheme, ReflowItem};
use noa_compositor::{
    canvas::{CanvasViewMut, Grapheme},
    compositor::Compositor,
    surface::{Layout, RectSize, Surface},
};

use crate::editor::Editor;

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
        self.compositor.add_frontmost_layer(Box::new(Text::new()));
        self.compositor.render_to_terminal(&mut self.editor);
        loop {
            tokio::select! {
                biased;

                Some(ev) = self.compositor.recv_terminal_event() => {
                    break;
                }
            }
        }
    }
}

struct Text {}

impl Text {
    pub fn new() -> Self {
        Text {}
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

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        let doc = editor.current_document();
        for ReflowItem {
            grapheme,
            grapheme_width,
            pos_in_screen: (canvas_y, canvas_x),
            pos_in_buffer,
        } in doc.reflow_iter(doc.top_left, canvas.width(), doc.editorconfig().tab_width)
        {
            if canvas_y >= canvas.height() {
                break;
            }

            match grapheme {
                PrintableGrapheme::Grapheme(grapheme) => {
                    canvas.write(
                        canvas_y,
                        canvas_x,
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
