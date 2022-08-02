use std::cmp::{max, min};

use noa_buffer::{
    cursor::Position,
    display_width::DisplayWidth,
    paragraph_iter::Paragraph,
    reflow_iter::{PrintableGrapheme, ReflowItem},
};
use noa_compositor::{
    canvas::{CanvasViewMut, Grapheme},
    compositor::Compositor,
    surface::{HandledEvent, KeyEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    actions::execute_action_or_notify,
    config::{get_keybinding_for, KeyBindingScope},
    editor::Editor,
    MainloopCommand,
};

pub struct BufferView {
    mainloop_tx: UnboundedSender<MainloopCommand>,
    virtual_buffer_width: usize,
    buffer_width: usize,
    buffer_height: usize,
    first_visible_pos: Position,
    last_visible_pos: Position,
    cursor_screen_pos: Option<(usize, usize)>,
    softwrap: bool,
}

impl BufferView {
    pub fn new(mainloop_tx: UnboundedSender<MainloopCommand>) -> Self {
        BufferView {
            mainloop_tx,
            virtual_buffer_width: 0,
            buffer_width: 0,
            buffer_height: 0,
            first_visible_pos: Position::new(0, 0),
            last_visible_pos: Position::new(0, 0),
            cursor_screen_pos: None,
            softwrap: true,
        }
    }
}

impl Surface for BufferView {
    type Context = Editor;

    fn name(&self) -> &str {
        "buffer"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _ctx: &mut Editor) -> bool {
        true
    }

    fn layout(
        &mut self,
        _ctx: &mut Editor,
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

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        self.cursor_screen_pos
    }

    fn handle_key_event(
        &mut self,
        editor: &mut Editor,
        compositor: &mut Compositor<Editor>,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.current_document_mut();

        let _show_completion = false;
        let mut adjust_scroll = true;
        // TODO: Move into defaults.toml
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.mainloop_tx.send(MainloopCommand::Quit);
            }
            (KeyCode::Esc, NONE) => {
                doc.clear_secondary_cursors();
            }
            (KeyCode::Up, NONE) => {
                doc.move_cursors_up(self.virtual_buffer_width);
            }
            (KeyCode::Down, NONE) => {
                doc.move_cursors_down(self.virtual_buffer_width);
            }
            (KeyCode::Left, NONE) => {
                doc.move_cursors_left();
            }
            (KeyCode::Right, NONE) => {
                doc.move_cursors_right();
            }
            (KeyCode::Up, SHIFT) => {
                doc.select_cursors_up(self.virtual_buffer_width);
            }
            (KeyCode::Down, SHIFT) => {
                doc.select_cursors_down(self.virtual_buffer_width);
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
            (KeyCode::Char('f'), CTRL) => {
                use std::process::Command;
                let mut cmd = Command::new("sk");
                cmd.arg("-c").arg("echo hello");
                self.mainloop_tx
                    .send(MainloopCommand::ExternalCommand(Box::new(cmd)));
            }
            (KeyCode::Char('r'), CTRL) => {
                self.softwrap = !self.softwrap;
                if self.softwrap {
                    self.virtual_buffer_width = self.buffer_width;
                } else {
                    self.virtual_buffer_width = usize::MAX;
                }
            }
            (KeyCode::PageUp, NONE) => {
                doc.scroll_up(self.buffer_height, self.virtual_buffer_width);
                adjust_scroll = false;
            }
            (KeyCode::PageDown, NONE) => {
                doc.scroll_down(self.buffer_height, self.virtual_buffer_width);
                adjust_scroll = false;
            }
            _ => {
                if let Some(binding) =
                    get_keybinding_for(KeyBindingScope::Buffer, key.code, key.modifiers)
                {
                    execute_action_or_notify(editor, compositor, &binding.action);
                }
            }
        }

        let doc = editor.current_document_mut();
        if adjust_scroll {
            doc.adjust_scroll(
                self.virtual_buffer_width,
                self.buffer_width,
                self.buffer_height,
                self.first_visible_pos,
                self.last_visible_pos,
            );
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        editor: &mut Editor,
        _compositor: &mut Compositor<Self::Context>,
        input: &str,
    ) -> HandledEvent {
        let doc = editor.current_document_mut();
        doc.insert(input);
        doc.adjust_scroll(
            self.virtual_buffer_width,
            self.buffer_width,
            self.buffer_height,
            self.first_visible_pos,
            self.last_visible_pos,
        );
        HandledEvent::Consumed
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();
        self.cursor_screen_pos = None;

        let doc = editor.current_document();

        self.buffer_height = canvas.height();
        let lineno_width =
            2 + (doc.scroll.paragraph_index.buffer_y + 1 + self.buffer_height).display_width();
        let buffer_start_x = lineno_width;
        self.buffer_width = canvas.width().saturating_sub(lineno_width);

        if self.buffer_width == 0 {
            warn!("too small screen");
            return;
        }

        if self.virtual_buffer_width != usize::MAX {
            self.virtual_buffer_width = self.buffer_width;
        }

        self.first_visible_pos = Position::new(usize::MAX, usize::MAX);
        self.last_visible_pos = Position::new(0, 0);

        // Buffer contents.
        let main_cursor_pos = doc.main_cursor().moving_position();
        let mut screen_y_offset = 0;
        let mut linenos: Vec<usize> = Vec::new();
        trace_timing!("render_text", 3 /* ms */, {
            for Paragraph {
                mut reflow_iter,
                index: paragraph_index,
            } in doc.paragraph_iter_at_index(
                doc.scroll.paragraph_index,
                self.virtual_buffer_width,
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

                    if pos_in_screen.x < doc.scroll.x_in_paragraph {
                        continue;
                    }

                    let canvas_y = screen_y_offset + pos_in_screen.y - skipped_y;
                    let canvas_x = buffer_start_x + pos_in_screen.x - doc.scroll.x_in_paragraph;

                    if canvas_y >= canvas.height() {
                        break;
                    }

                    if canvas_x >= canvas.width() {
                        break;
                    }

                    self.first_visible_pos = min(self.first_visible_pos, pos_in_buffer);
                    self.last_visible_pos = max(self.last_visible_pos, pos_in_buffer);

                    if canvas_y >= linenos.len() {
                        linenos.push(pos_in_buffer.y + 1);
                    }

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
                            canvas.set_inverted(
                                canvas_y,
                                canvas_x,
                                canvas_x + grapheme_width,
                                true,
                            );
                        }
                    }

                    if main_cursor_pos == pos_in_buffer {
                        self.cursor_screen_pos = Some((canvas_y, canvas_x));
                    }
                }

                screen_y_offset += 1 + paragraph_height - skipped_y;
            }
        });

        // Line numbers.
        let mut prev = 0;
        for (canvas_y, lineno) in linenos.into_iter().enumerate() {
            if lineno != prev {
                canvas.write_str(
                    canvas_y,
                    1,
                    &format!("{:>width$}", lineno, width = lineno_width - 2),
                );
            }

            prev = lineno;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::document::Document;

    use super::*;
    use noa_compositor::canvas::Canvas;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;

    #[test]
    fn with_softwrap() {
        let (mainloop_tx, mainloop_rx) = mpsc::unbounded_channel();
        let mut view = BufferView::new(mainloop_tx);

        // 1 abcdefghijklmn
        let mut editor = Editor::new();
        editor.add_and_switch_document(Document::virtual_file("test.txt", "abcdefg"));
        let mut canvas = Canvas::new(10, 8);
        view.render(&mut editor, &mut canvas.view_mut());
        insta::assert_debug_snapshot!(canvas.graphemes());
    }
}
