use std::cmp::{max, min};

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
            trace_timing!("render", 10 /* ms */, {
                self.compositor.render(&mut self.editor);
            });

            tokio::select! {
                biased;

                Some(command) = mainloop_rx.recv() => {
                    match command {
                        MainloopCommand::Quit => break,
                    }
                }

                Some(ev) = self.compositor.receive_event() => {
                    self.compositor.handle_event(&mut self.editor, ev);
                }
            }
        }
    }
}

struct Text {
    mainloop_tx: UnboundedSender<MainloopCommand>,
    buffer_width: usize,
    buffer_height: usize,
    first_visible_pos: Position,
    last_visible_pos: Position,
    cursor_screen_pos: Option<(usize, usize)>,
}

impl Text {
    pub fn new(mainloop_tx: UnboundedSender<MainloopCommand>) -> Self {
        Text {
            mainloop_tx,
            buffer_width: 0,
            buffer_height: 0,
            first_visible_pos: Position::new(0, 0),
            last_visible_pos: Position::new(0, 0),
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
            screen_size,
            // RectSize {
            //     width: screen_size.width,
            //     height: screen_size.height.saturating_sub(2),
            // },
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
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.current_document_mut();

        let mut show_completion = false;
        let mut adjust_scroll = true;
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.mainloop_tx.send(MainloopCommand::Quit);
            }
            (KeyCode::Esc, NONE) => {
                doc.clear_secondary_cursors();
            }
            (KeyCode::Up, NONE) => {
                // doc.move_cursors_up(self.buffer_width);
                doc.scroll_up(1, self.buffer_width);
                adjust_scroll = false;
            }
            (KeyCode::Down, NONE) => {
                // doc.move_cursors_down(self.buffer_width);
                doc.scroll_down(1, self.buffer_width);
                adjust_scroll = false;
            }
            (KeyCode::Left, NONE) => {
                doc.move_cursors_left();
            }
            (KeyCode::Right, NONE) => {
                doc.move_cursors_right();
            }
            (KeyCode::Up, SHIFT) => {
                doc.select_cursors_up(self.buffer_width);
            }
            (KeyCode::Down, SHIFT) => {
                doc.select_cursors_down(self.buffer_width);
            }
            (KeyCode::Left, SHIFT) => {
                doc.select_cursors_left();
            }
            (KeyCode::Right, SHIFT) => {
                doc.select_cursors_right();
            }
            (KeyCode::Char(ch), NONE) => {
                doc.smart_insert_char(ch);
            }
            (KeyCode::Enter, NONE) => {
                doc.smart_insert_char('\n');
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
            (KeyCode::Char('a'), CTRL) => {
                doc.move_to_beginning_of_line();
            }
            (KeyCode::Char('e'), CTRL) => {
                doc.move_to_end_of_line();
            }
            (KeyCode::Char('f'), ALT) => {
                doc.move_to_next_word();
            }
            (KeyCode::Char('b'), ALT) => {
                doc.move_to_prev_word();
            }
            _ => {}
        }

        if adjust_scroll {
            doc.adjust_scroll(
                self.buffer_width,
                self.buffer_height,
                self.first_visible_pos,
                self.last_visible_pos,
            );
        }

        HandledEvent::Consumed
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();
        self.cursor_screen_pos = None;
        self.buffer_width = canvas.width();
        self.buffer_height = canvas.height();
        self.first_visible_pos = Position::new(usize::MAX, usize::MAX);
        self.last_visible_pos = Position::new(0, 0);

        let doc = editor.current_document();
        let main_cursor_pos = doc.main_cursor().moving_position();
        let mut screen_y_offset = 0;
        for (Paragraph {
            mut reflow_iter,
            index: paragraph_index,
        }) in doc.paragraph_iter_at_index(
            doc.scroll.paragraph_index,
            self.buffer_width,
            doc.editorconfig().tab_width,
        ) {
            reflow_iter.enable_eof(true);

            let mut paragraph_height = 0;
            let skipped_y = if doc.scroll.paragraph_index == paragraph_index {
                doc.scroll.y_in_paragraph
            } else {
                0
            };

            for ReflowItem {
                grapheme,
                grapheme_width,
                pos_in_screen,
                pos_in_buffer,
            } in reflow_iter
            {
                if pos_in_screen.y < skipped_y {
                    continue;
                }

                let canvas_y = screen_y_offset + pos_in_screen.y - skipped_y;
                let canvas_x = pos_in_screen.x;

                if canvas_y >= canvas.height() {
                    break;
                }

                if pos_in_screen.x < doc.scroll.x_in_paragraph {
                    continue;
                }

                // info!(
                //     "yx=({}, {}), canvas_y({}, {}), g={:?}, paragraph_screen_y={}",
                //     pos_in_screen.y, pos_in_screen.x, canvas_y, canvas_x, grapheme, screen_y_offset
                // );

                self.first_visible_pos = min(self.first_visible_pos, pos_in_buffer);
                self.last_visible_pos = max(self.last_visible_pos, pos_in_buffer);

                match grapheme {
                    PrintableGrapheme::Grapheme(grapheme) => {
                        paragraph_height = pos_in_screen.y;
                        canvas.write(
                            canvas_y,
                            canvas_x,
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

                for c in doc.cursors() {
                    if c.selection().contains(pos_in_buffer)
                        || (!c.is_main_cursor() && c.position() == Some(pos_in_buffer))
                    {
                        canvas.set_inverted(canvas_y, canvas_x, canvas_x + grapheme_width, true);
                    }
                }

                if main_cursor_pos == pos_in_buffer {
                    self.cursor_screen_pos = Some((canvas_y, canvas_x));
                }
            }

            screen_y_offset += 1 + paragraph_height - skipped_y;
        }
    }
}
