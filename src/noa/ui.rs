use noa_buffer::{
    cursor::Position,
    paragraph_iter::Paragraph,
    reflow_iter::{PrintableGrapheme, ReflowItem},
};
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
    buffer_width: usize,
    cursor_screen_pos: Option<(usize, usize)>,
}

impl Text {
    pub fn new(mainloop_tx: UnboundedSender<MainloopCommand>) -> Self {
        Text {
            mainloop_tx,
            buffer_width: 0,
            cursor_screen_pos: None,
        }
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

    fn is_active(&self, ctx: &mut Editor) -> bool {
        true
    }

    fn layout(
        &mut self,
        ctx: &mut Editor,
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

    fn cursor_position(&self, editor: &mut Editor) -> Option<(usize, usize)> {
        self.cursor_screen_pos
    }

    fn handle_key_event(
        &mut self,
        editor: &mut Editor,
        _compositor: &mut Compositor<Editor>,
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
            (KeyCode::Left, NONE) => {
                doc.move_cursors_left();
            }
            (KeyCode::Right, NONE) => {
                doc.move_cursors_right();
            }
            (KeyCode::Up, NONE) => {
                doc.move_cursors_up(self.buffer_width);
            }
            (KeyCode::Down, NONE) => {
                doc.move_cursors_down(self.buffer_width);
            }
            (KeyCode::Char(ch), NONE) => {
                doc.smart_insert_char(ch);
            }
            (KeyCode::Char(ch), SHIFT) => {
                doc.smart_insert_char(ch.to_ascii_uppercase());
            }
            (KeyCode::Backspace, NONE) => {
                doc.backspace();
            }
            (KeyCode::Delete, NONE) => {
                doc.delete();
            }
            _ => {}
        }

        HandledEvent::Consumed
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();
        self.buffer_width = canvas.width();

        let doc = editor.current_document();
        let main_cursor = doc.main_cursor();
        let mut paragraph_screen_y = 0;
        for (Paragraph { mut reflow_iter }) in doc.paragraph_iter(
            doc.scroll.buf_pos,
            self.buffer_width,
            doc.editorconfig().tab_width,
        ) {
            reflow_iter.enable_eof(true);
            let mut paragraph_height = 0;
            for ReflowItem {
                grapheme,
                grapheme_width,
                pos_in_screen,
                pos_in_buffer,
            } in reflow_iter
            {
                if pos_in_screen.y >= canvas.height() {
                    break;
                }

                if pos_in_screen.y < doc.scroll.screen_pos.y {
                    continue;
                }

                if pos_in_screen.x < doc.scroll.screen_pos.x {
                    continue;
                }

                info!("grapheme = {:?}", grapheme);
                match grapheme {
                    PrintableGrapheme::Grapheme(grapheme) => {
                        paragraph_height = pos_in_screen.y;
                        canvas.write(
                            paragraph_screen_y + pos_in_screen.y,
                            pos_in_screen.x,
                            Grapheme::new_with_width(grapheme, grapheme_width),
                        );
                    }
                    PrintableGrapheme::Eof
                    | PrintableGrapheme::Whitespaces
                    | PrintableGrapheme::ZeroWidth
                    | PrintableGrapheme::Newline(_) => {
                        // Already filled with whitespaces by `canvas.clear()`.
                    }
                }

                info!(
                    "paragraph_screen_y + pos_in_screen.y = {:?}, {:?}",
                    paragraph_screen_y, pos_in_screen.y
                );
                if main_cursor.moving_position() == pos_in_buffer {
                    info!("main_cursor: {pos_in_buffer}");
                    self.cursor_screen_pos =
                        Some((paragraph_screen_y + pos_in_screen.y, pos_in_screen.x));
                }
            }

            paragraph_screen_y += 1 + paragraph_height;
        }
    }
}
