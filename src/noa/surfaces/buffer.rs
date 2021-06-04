use std::cmp::min;

use anyhow::Result;
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    style::Color,
};

use crate::terminal::{compositor::Canvas, display_width::DisplayWidth};

use super::{Context, Surface};

fn whitespaces(n: usize) -> String {
    " ".repeat(n)
}

pub fn truncate(s: &str, width: usize) -> &str {
    &s[..min(s.chars().count(), width)]
}

pub struct BufferSurface {}

impl BufferSurface {
    pub fn new() -> BufferSurface {
        BufferSurface {}
    }
}

impl Surface for BufferSurface {
    fn is_invalidated(&self, ctx: &mut Context) -> bool {
        true
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) -> Result<()> {
        let lineno_width = ctx.buffer.num_lines().display_width() + 1;
        ctx.view
            .layout(&ctx.buffer, 0, canvas.width(), canvas.height());

        for (y, display_line) in ctx.view.visible_display_lines().iter().enumerate() {
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
            let rope_line = ctx.buffer.line(lineno - 1);
            for chunk in &display_line.chunks {
                let chunk_str = rope_line.slice(chunk.clone());
                let mut x = 0;
                for s in chunk_str.chunks() {
                    for ch in s.chars() {
                        canvas.set_char(y, text_start + x, ch);
                        x += 1;
                    }
                }

                for x in (text_start + x)..canvas.width() {
                    canvas.set_char(y, x, ' ');
                }
            }
        }

        Ok(())
    }

    fn handle_key_event(&mut self, ctx: &mut Context, key: KeyEvent) -> Result<()> {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
        let ctrl_alt = KeyModifiers::CONTROL | KeyModifiers::ALT;

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                *ctx.exited = true;
            }
            (KeyCode::Backspace, NONE) => {
                ctx.buffer.backspace();
            }
            (KeyCode::Up, NONE) => {
                ctx.buffer.move_cursors(1, 0, 0, 0);
            }
            (KeyCode::Down, NONE) => {
                ctx.buffer.move_cursors(0, 1, 0, 0);
            }
            (KeyCode::Left, NONE) => {
                ctx.buffer.move_cursors(0, 0, 1, 0);
            }
            (KeyCode::Right, NONE) => {
                ctx.buffer.move_cursors(0, 0, 0, 1);
            }
            (KeyCode::Enter, NONE) => {
                ctx.buffer.insert_char('\n');
            }
            (KeyCode::Char(ch), NONE) => {
                ctx.buffer.insert_char(ch);
            }
            _ => {
                trace!("unhandled key = {:?}", key);
            }
        }

        Ok(())
    }

    fn handle_key_batch_event(&mut self, ctx: &mut Context, input: &str) -> Result<()> {
        ctx.buffer.insert(&input);
        Ok(())
    }
}
