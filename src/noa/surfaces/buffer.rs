use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
        let buffer = ctx.editor.current_buffer().read();
        let view = ctx
            .editor
            .compute_view(&*buffer, canvas.height(), canvas.width());
        let lineno_width = buffer.num_lines().display_width() + 1;

        for (y, display_line) in view.visible_display_lines().iter().enumerate() {
            // Draw the line number.
            let lineno = display_line.range.front().y + 1;
            let pad_len = lineno_width - lineno.display_width() - 1;
            canvas.set_str(y, 0, &whitespaces(pad_len));
            canvas.set_str(y, pad_len, &lineno.to_string());
            canvas.set_char(
                y,
                pad_len + lineno_width,
                '\u{2502}', /* "Box Drawing Light Veritical" */
            );

            // Draw buffer contents.
            let text_start = pad_len + lineno_width + 1;
            let rope_line = buffer.line(lineno - 1);
            for chunk in &display_line.chunks {
                let chunk_str = rope_line.slice(chunk.clone());
                let mut x = 0;
                for s in chunk_str.chunks() {
                    for ch in s.chars() {
                        canvas.set_char(y, text_start + x, ch);
                        x += 1;
                    }
                }

                canvas.set_str(y, x, &whitespaces(canvas.width() - (text_start + x)));
            }
        }

        Ok(())
    }

    fn handle_key_event(&mut self, ctx: &mut Context, key: KeyEvent) -> Result<()> {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
        let _ctrl_alt = KeyModifiers::CONTROL | KeyModifiers::ALT;

        let mut buffer = ctx.editor.current_buffer().write();
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                drop(buffer);
                ctx.editor.exit_editor();
            }
            (KeyCode::Backspace, NONE) => {
                buffer.backspace();
            }
            (KeyCode::Up, NONE) => {
                buffer.move_cursors(1, 0, 0, 0);
            }
            (KeyCode::Down, NONE) => {
                buffer.move_cursors(0, 1, 0, 0);
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
