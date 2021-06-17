use std::cmp::{max, min};

use anyhow::Result;
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    style::{Attribute, Attributes},
};
use noa_buffer::{Cursor, Range};

use crate::{
    surfaces::{prompt::CallbackResult, yes_no::YesNoChoice, FinderSurface, YesNoSurface},
    ui::{
        whitespaces, Canvas, Compositor, Context, DisplayWidth, HandledEvent, Layout, RectSize,
        Surface,
    },
};

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

    fn quit(&mut self, ctx: &mut Context, compositor: &mut Compositor) {
        // Check if all buffers are not dirty.
        let mut num_unsaved_files = 0;
        let mut example = Some("".to_owned());
        for buffer in ctx.editor.buffers() {
            let buffer = buffer.read();
            if buffer.is_dirty() {
                if let Some(path) = buffer.path() {
                    let filename = path.file_name().unwrap().to_str().unwrap().to_owned();
                    num_unsaved_files += 1;
                    example = Some(filename);
                }
            }
        }

        if num_unsaved_files == 0 {
            ctx.editor.exit_editor();
            return;
        }

        // If any files are not yet saved, show a dialog to ask what we should do.
        let title = format!(
            "{} unsaved files: e.g. {}",
            num_unsaved_files,
            example.unwrap()
        );
        let prompt = YesNoSurface::new(
            ctx,
            &title,
            vec![
                // Save all.
                YesNoChoice::new('a', |ctx| {
                    for buffer in ctx.editor.buffers() {
                        let mut buffer = buffer.write();
                        if !buffer.is_virtual_file() {
                            buffer.save(ctx.editor.backup_dir());
                        }
                    }
                    ctx.editor.exit_editor();
                    CallbackResult::Close
                }),
                // Cancel.
                YesNoChoice::new('c', |ctx| CallbackResult::Close),
                // Force quit.
                YesNoChoice::new('Q', |ctx| {
                    ctx.editor.exit_editor();
                    CallbackResult::Close
                }),
            ],
        );
        compositor.push_layer(ctx, prompt);
    }
}

impl Surface for BufferSurface {
    fn name(&self) -> &str {
        "buffer"
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        (Layout::Full, screen_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        Some(self.cursor_position)
    }

    fn render(&mut self, ctx: &mut Context, canvas: &mut Canvas) {
        canvas.clear();

        let buffer = ctx.editor.current_buffer().read();
        let view = ctx
            .editor
            .compute_view(&*buffer, canvas.height(), canvas.width());

        let max_lineno_width = buffer.num_lines().display_width() + 1;
        let text_start = max_lineno_width + 1;

        let mut y_end = 0;
        let mut lines_end_xs = Vec::new();
        for (y, display_line) in view.visible_display_lines().iter().enumerate() {
            // Draw the line number.
            let lineno = display_line.range.front().y + 1;
            let lineno_width = lineno.display_width();
            let pad_len = max_lineno_width - lineno_width;
            canvas.draw_str(y, 0, &whitespaces(pad_len));
            canvas.draw_str(y, pad_len, &lineno.to_string());
            canvas.draw_char(
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
                        canvas.draw_char(y, text_start + x, ch);
                        x += 1;
                    }
                }
            }

            canvas.draw_str(
                y,
                text_start + x,
                &whitespaces(canvas.width() - (text_start + x)),
            );

            lines_end_xs.push(x);
            y_end = y + 1;
        }

        // Clear the remaining out of the buffer area.
        for y in y_end..canvas.height() {
            canvas.draw_str(y, 0, &whitespaces(canvas.width()));
        }

        // Draw cursors / selections.
        let main_cursor_pos = buffer.main_cursor_pos();
        for cursor in buffer.cursors() {
            match cursor {
                Cursor::Normal { pos, .. } if *pos == main_cursor_pos => {
                    // Do nothing. We use the native cursor through `self.cursor_position`.
                }
                Cursor::Normal { pos, .. } => {
                    let (y, x) = view.point_to_display_pos(
                        main_cursor_pos,
                        y_end,
                        text_start,
                        buffer.num_lines(),
                    );
                    canvas.set_attrs(y, x, y, x + 1, (&[Attribute::Reverse][..]).into());
                }
                Cursor::Selection(range) => {
                    let (start_y, start_x) = view.point_to_display_pos(
                        range.front(),
                        y_end,
                        text_start,
                        buffer.num_lines(),
                    );
                    let (end_y, end_x) = view.point_to_display_pos(
                        range.back(),
                        y_end,
                        text_start,
                        buffer.num_lines(),
                    );

                    for (y, display_line) in view.visible_display_lines().iter().enumerate() {
                        if start_y <= y && y <= end_y {
                            let x0 = if y == start_y { start_x } else { text_start };
                            let x1 = if y == end_y {
                                end_x
                            } else {
                                text_start + lines_end_xs[y] + 1
                            };
                            canvas.set_attrs(
                                y,
                                min(x0, x1),
                                y + 1,
                                max(x0, x1),
                                (&[Attribute::Reverse][..]).into(),
                            );
                        }
                    }
                }
            }
        }

        // Determine the main cursor position.
        self.cursor_position =
            view.point_to_display_pos(main_cursor_pos, y_end, text_start, buffer.num_lines());
    }

    fn handle_key_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let mut buffer = ctx.editor.current_buffer().write();
        let view = ctx.editor.view(&*buffer);
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                drop(buffer);
                drop(view);
                self.quit(ctx, compositor);
            }
            (KeyCode::Char('f'), CTRL) => {
                drop(buffer);
                drop(view);
                let finder = FinderSurface::new(ctx);
                compositor.push_layer(ctx, finder);
            }
            (KeyCode::Char('s'), CTRL) => {
                buffer.save(ctx.editor.backup_dir());
            }
            (KeyCode::Char('u'), CTRL) => {
                buffer.undo();
            }
            (KeyCode::Char('y'), CTRL) => {
                buffer.redo();
            }
            (KeyCode::Char('d'), CTRL) => {
                buffer.delete();
            }
            (KeyCode::Char('k'), CTRL) => {
                buffer.truncate();
            }
            (KeyCode::Char('a'), CTRL) => {
                buffer.move_to_beginning_of_line();
            }
            (KeyCode::Char('e'), CTRL) => {
                buffer.move_to_end_of_line();
            }
            (KeyCode::Char('f'), ALT) => {
                buffer.move_to_next_word();
            }
            (KeyCode::Char('b'), ALT) => {
                buffer.move_to_prev_word();
            }
            (KeyCode::Up, ALT) => {
                buffer.move_current_line_above();
            }
            (KeyCode::Down, ALT) => {
                buffer.move_current_line_below();
            }
            (KeyCode::Up, modifiers) if modifiers == (CTRL | ALT) => {
                buffer.add_cursor_above();
            }
            (KeyCode::Down, modifiers) if modifiers == (CTRL | ALT) => {
                buffer.add_cursor_below();
            }
            (KeyCode::Up, modifiers) if modifiers == (SHIFT | ALT) => {
                buffer.duplicate_line_above();
            }
            (KeyCode::Down, modifiers) if modifiers == (SHIFT | ALT) => {
                buffer.duplicate_line_below();
            }
            (KeyCode::Backspace, NONE) => {
                buffer.backspace();
            }
            (KeyCode::Up, NONE) => {
                view.move_cursors(&mut *buffer, -1, 0);
            }
            (KeyCode::Down, NONE) => {
                view.move_cursors(&mut *buffer, 1, 0);
            }
            (KeyCode::Left, NONE) => {
                view.move_cursors(&mut *buffer, 0, -1);
            }
            (KeyCode::Right, NONE) => {
                view.move_cursors(&mut *buffer, 0, 1);
            }
            (KeyCode::Up, SHIFT) => {
                view.expand_selections(&mut *buffer, -1, 0);
            }
            (KeyCode::Down, SHIFT) => {
                view.expand_selections(&mut *buffer, 1, 0);
            }
            (KeyCode::Left, SHIFT) => {
                view.expand_selections(&mut *buffer, 0, -1);
            }
            (KeyCode::Right, SHIFT) => {
                view.expand_selections(&mut *buffer, 0, 1);
            }
            (KeyCode::Enter, NONE) => {
                buffer.insert_char('\n');
            }
            (KeyCode::Tab, NONE) => {
                buffer.tab();
            }
            (KeyCode::BackTab, NONE) => {
                buffer.back_tab();
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

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        ctx: &mut Context,
        _compositor: &mut Compositor,
        input: &str,
    ) -> HandledEvent {
        ctx.editor.current_buffer().write().insert(&input);
        HandledEvent::Consumed
    }
}
