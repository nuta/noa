use anyhow::Result;
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    style::{Attribute, Attributes},
};
use noa_buffer::{Cursor, Range};

use crate::terminal::{canvas::Canvas, display_width::DisplayWidth};

use super::{whitespaces, Context, Surface};

pub struct BufferSurface {
    // `(y, x)`.
    cursor_position: (usize, usize),
}

impl BufferSurface {
    pub fn new() -> BufferSurface {
        BufferSurface {
            cursor_position: (0, 0),
        }
    }
}

impl Surface for BufferSurface {
    fn name(&self) -> &str {
        "buffer"
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        Some(self.cursor_position)
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()> {
        self.render_all(ctx, canvas)
    }

    fn render_all(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()> {
        canvas.clear();

        let buffer = ctx.editor.current_buffer().read();
        let view = ctx
            .editor
            .compute_view(&*buffer, canvas.height(), canvas.width());

        let max_lineno_width = buffer.num_lines().display_width() + 1;
        let text_start = max_lineno_width + 1;

        let mut y_end = 0;
        for (y, display_line) in view.visible_display_lines().iter().enumerate() {
            // Draw the line number.
            let lineno = display_line.range.front().y + 1;
            let lineno_width = lineno.display_width();
            let pad_len = max_lineno_width - lineno_width;
            canvas.set_str(y, 0, &whitespaces(pad_len));
            canvas.set_str(y, pad_len, &lineno.to_string());
            canvas.set_char(
                y,
                max_lineno_width,
                '\u{2502}', /* "Box Drawing Light Veritical" */
            );

            // Draw buffer contents.
            let rope_line = buffer.line(lineno - 1);
            let mut x = 0;
            for chunk in &display_line.chunks {
                let chunk_str = rope_line.slice(chunk.clone());
                for s in chunk_str.chunks() {
                    for ch in s.chars() {
                        canvas.set_char(y, text_start + x, ch);
                        x += 1;
                    }
                }
            }

            canvas.set_str(
                y,
                text_start + x,
                &whitespaces(canvas.width() - (text_start + x)),
            );

            y_end = y + 1;
        }

        // Clear the remaining out of the buffer area.
        for y in y_end..canvas.height() {
            canvas.set_str(y, 0, &whitespaces(canvas.width()));
        }

        // Draw cursors / selections.
        let main_cursor_pos = buffer.main_cursor_pos();
        for cursor in buffer.cursors() {
            match cursor {
                Cursor::Normal { pos } if pos == main_cursor_pos => {
                    // Do nothing. We use the native cursor through `self.cursor_position`.
                }
                Cursor::Normal { pos } => {
                    let (y, x) = view.point_to_display_pos(
                        main_cursor_pos,
                        y_end,
                        text_start,
                        buffer.num_lines(),
                    );
                    canvas.add_attrs(y, x, y, x + 1, (&[Attribute::Reverse][..]).into());
                }
                Cursor::Selection(Range { start, end }) => {
                    for buffer_y in start.y..end.y {
                        let (y, x) = view.point_to_display_pos(
                            main_cursor_pos,
                            y_end,
                            text_start,
                            buffer.num_lines(),
                        );

                        canvas.add_attrs(y, x, y, x + 1, (&[Attribute::Reverse][..]).into());
                    }
                }
            }
        }

        // Determine the main cursor position.
        self.cursor_position =
            view.point_to_display_pos(main_cursor_pos, y_end, text_start, buffer.num_lines());
        Ok(())
    }

    fn handle_key_event(&mut self, ctx: &mut Context, key: KeyEvent) -> Result<()> {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
        let _ctrl_alt = KeyModifiers::CONTROL | KeyModifiers::ALT;

        let mut buffer = ctx.editor.current_buffer().write();
        let view = ctx.editor.view(&*buffer);
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                drop(buffer);
                drop(view);
                ctx.editor.exit_editor();
            }
            (KeyCode::Backspace, NONE) => {
                buffer.backspace();
            }
            (KeyCode::Up, NONE) => {
                view.move_cursors_vertically(&mut *buffer, -1);
            }
            (KeyCode::Down, NONE) => {
                view.move_cursors_vertically(&mut *buffer, 1);
            }
            (KeyCode::Left, NONE) => {
                buffer.move_cursors(0, 0, 1, 0);
            }
            (KeyCode::Right, NONE) => {
                buffer.move_cursors(0, 0, 0, 1);
            }
            (KeyCode::Enter, NONE) => {
                buffer.insert_char('\n');
            }
            (KeyCode::Char(ch), NONE) => {
                buffer.insert_char(ch);
            }
            (KeyCode::Char(ch), SHIFT) => {
                buffer.insert_char(ch);
            }
            _ => {
                trace!("unhandled key = {:?}", key);
            }
        }

        Ok(())
    }

    fn handle_key_batch_event(&mut self, ctx: &mut Context, input: &str) -> Result<()> {
        ctx.editor.current_buffer().write().insert(&input);
        Ok(())
    }
}
