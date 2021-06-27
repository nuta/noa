use std::cmp::{max, min};

use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    style::{Attribute, Color},
};
use noa_buffer::{Cursor, Point, Range};
use noa_langs::HighlightType;

use crate::{
    surfaces::{prompt::CallbackResult, yes_no::YesNoChoice, FinderSurface, YesNoSurface},
    terminal::copy_to_clipboard,
    ui::{
        whitespaces, CanvasViewMut, Compositor, Context, DisplayWidth, HandledEvent, Layout,
        RectSize, Surface,
    },
};

pub struct BufferSurface {
    // `(y, x)`.
    cursor_position: (usize, usize),
    text_start_x: usize,
    selection_start: Option<Point>,
}

impl BufferSurface {
    pub fn new() -> BufferSurface {
        BufferSurface {
            cursor_position: (0, 0),
            text_start_x: 0,
            selection_start: None,
        }
    }

    fn quit(&mut self, ctx: &mut Context, compositor: &mut Compositor) {
        let dirty_buffers = ctx.editor.dirty_buffers();
        if dirty_buffers.is_empty() {
            ctx.editor.exit_editor();
            return;
        }

        // If any files are not yet saved, show a dialog to ask what we should do.
        let first_buffer = dirty_buffers[0].read();
        let basename = first_buffer
            .buffer
            .path()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let title = format!(
            "{} unsaved files ({}{})",
            dirty_buffers.len(),
            basename,
            if dirty_buffers.len() > 1 { ", ..." } else { "" }
        );
        let prompt = YesNoSurface::new(
            ctx,
            &title,
            vec![
                // Save all.
                YesNoChoice::new('a', |ctx| {
                    ctx.editor.save_all();
                    ctx.editor.exit_editor();
                    CallbackResult::Close
                }),
                // Cancel.
                YesNoChoice::new('c', |_ctx| CallbackResult::Close),
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
        (
            Layout::Fixed { y: 0, x: 0 },
            RectSize {
                width: screen_size.width,
                height: screen_size.height.saturating_sub(2 /* bottom bar */),
            },
        )
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        Some(self.cursor_position)
    }

    fn render<'a>(&mut self, ctx: &mut Context, mut canvas: CanvasViewMut<'a>) {
        canvas.clear();

        {
            let mut f = ctx.editor.current_file().write();
            f.layout_view(0, canvas.height(), canvas.width());
            f.highlight_from_tree_sitter();
        }

        let f = ctx.editor.current_file().read();

        let max_lineno_width = f.buffer.num_lines().display_width() + 1;
        let text_start_x = max_lineno_width + 1;

        let mut y_end = 0;
        let mut lines_end_xs = Vec::new();
        for (y, display_line) in f.view.visible_display_lines().iter().enumerate() {
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
            let rope_line = f.buffer.line(lineno - 1);
            let mut x = 0;
            for chunk in &display_line.chunks {
                let chunk_str = rope_line.slice(chunk.clone());
                for s in chunk_str.chunks() {
                    for ch in s.chars() {
                        canvas.draw_char(y, text_start_x + x, ch);
                        x += 1;
                    }
                }
            }

            // Highlights.
            for h in &display_line.highlights {
                let x_start = text_start_x + h.range.start;
                let x_end = text_start_x + h.range.end;
                let color = match h.highlight_type {
                    HighlightType::Ident => Color::Magenta,
                    HighlightType::StringLiteral => Color::Green,
                    HighlightType::EscapeSequence => Color::Cyan,
                    HighlightType::PrimitiveType => Color::Cyan,
                    HighlightType::CMacro => Color::Magenta,
                    HighlightType::CIncludeArg => Color::Green,
                };

                canvas.set_fg(y, x_start, x_end, color);
            }

            // Whitespaces after the line.
            canvas.draw_str(
                y,
                text_start_x + x,
                &whitespaces(canvas.width() - (text_start_x + x)),
            );

            lines_end_xs.push(x);
            y_end = y + 1;
        }

        // Clear the remaining out of the buffer area.
        for y in y_end..canvas.height() {
            canvas.draw_str(y, 0, &whitespaces(canvas.width()));
        }

        // Draw cursors / selections.
        let main_cursor_pos = f.buffer.main_cursor_pos();
        for cursor in f.buffer.cursors() {
            match cursor {
                Cursor::Normal { pos, .. } if *pos == main_cursor_pos => {
                    // Do nothing. We use the native cursor through `self.cursor_position`.
                }
                Cursor::Normal { pos: _, .. } => {
                    let (y, x) = f.view.point_to_display_pos(
                        main_cursor_pos,
                        y_end,
                        text_start_x,
                        f.buffer.num_lines(),
                    );
                    canvas.set_attrs(y, x, x + 1, (&[Attribute::Reverse][..]).into());
                }
                Cursor::Selection(range) => {
                    let (start_y, start_x) = f.view.point_to_display_pos(
                        range.front(),
                        y_end,
                        text_start_x,
                        f.buffer.num_lines(),
                    );
                    let (end_y, end_x) = f.view.point_to_display_pos(
                        range.back(),
                        y_end,
                        text_start_x,
                        f.buffer.num_lines(),
                    );

                    for (y, _display_line) in f.view.visible_display_lines().iter().enumerate() {
                        if start_y <= y && y <= end_y {
                            let x0 = if y == start_y { start_x } else { text_start_x };
                            let x1 = if y == end_y {
                                end_x
                            } else {
                                text_start_x + lines_end_xs[y] + 1
                            };
                            canvas.set_attrs(
                                y,
                                min(x0, x1),
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
            f.view
                .point_to_display_pos(main_cursor_pos, y_end, text_start_x, f.buffer.num_lines());

        self.text_start_x = text_start_x;
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

        let mut f = ctx.editor.current_file().write();

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                drop(f);
                self.quit(ctx, compositor);
            }
            (KeyCode::Char('f'), CTRL) => {
                drop(f);
                let finder = FinderSurface::new(ctx);
                compositor.push_layer(ctx, finder);
            }
            (KeyCode::Char('s'), CTRL) => {
                drop(f);
                ctx.editor.save_current_buffer();
            }
            (KeyCode::Char('u'), CTRL) => {
                f.buffer.undo();
            }
            (KeyCode::Char('y'), CTRL) => {
                f.buffer.redo();
            }
            (KeyCode::Char('d'), CTRL) | (KeyCode::Delete, _) => {
                f.buffer.delete();
            }
            (KeyCode::Char('c'), CTRL) => {
                copy_to_clipboard(&f.buffer.copy_selection());
            }
            (KeyCode::Char('x'), CTRL) => {
                copy_to_clipboard(&f.buffer.cut_selection());
            }
            (KeyCode::Char('k'), CTRL) => {
                f.buffer.truncate();
            }
            (KeyCode::Char('a'), CTRL) => {
                f.buffer.move_to_beginning_of_line();
            }
            (KeyCode::Char('e'), CTRL) => {
                f.buffer.move_to_end_of_line();
            }
            (KeyCode::Char('f'), ALT) => {
                f.buffer.move_to_next_word();
            }
            (KeyCode::Char('b'), ALT) => {
                f.buffer.move_to_prev_word();
            }
            (KeyCode::Up, ALT) => {
                f.buffer.move_current_line_above();
            }
            (KeyCode::Down, ALT) => {
                f.buffer.move_current_line_below();
            }
            (KeyCode::Up, modifiers) if modifiers == (CTRL | ALT) => {
                f.buffer.add_cursor_above();
            }
            (KeyCode::Down, modifiers) if modifiers == (CTRL | ALT) => {
                f.buffer.add_cursor_below();
            }
            (KeyCode::Up, modifiers) if modifiers == (SHIFT | ALT) => {
                f.buffer.duplicate_line_above();
            }
            (KeyCode::Down, modifiers) if modifiers == (SHIFT | ALT) => {
                f.buffer.duplicate_line_below();
            }
            (KeyCode::Backspace, NONE) => {
                f.buffer.backspace();
            }
            (KeyCode::Up, NONE) => {
                f.move_cursors(-1, 0);
            }
            (KeyCode::Down, NONE) => {
                f.move_cursors(1, 0);
            }
            (KeyCode::Left, NONE) => {
                f.move_cursors(0, -1);
            }
            (KeyCode::Right, NONE) => {
                f.move_cursors(0, 1);
            }
            (KeyCode::Up, SHIFT) => {
                f.expand_selections(-1, 0);
            }
            (KeyCode::Down, SHIFT) => {
                f.expand_selections(1, 0);
            }
            (KeyCode::Left, SHIFT) => {
                f.expand_selections(0, -1);
            }
            (KeyCode::Right, SHIFT) => {
                f.expand_selections(0, 1);
            }
            (KeyCode::Enter, NONE) => {
                f.buffer.insert_char('\n');
            }
            (KeyCode::Tab, NONE) => {
                f.buffer.tab();
            }
            (KeyCode::BackTab, NONE) => {
                f.buffer.back_tab();
            }
            (KeyCode::Char(ch), NONE) => {
                f.buffer.insert_char(ch);
            }
            (KeyCode::Char(ch), SHIFT) => {
                f.buffer.insert_char(ch);
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
        ctx.editor.current_file().write().buffer.insert(input);
        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        ctx: &mut Context,
        _compositor: &mut Compositor,
        ev: MouseEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;

        let mut f = ctx.editor.current_file().write();

        let MouseEvent {
            kind,
            column: display_x,
            row: display_y,
            modifiers,
        } = ev;

        let buffer_pos = match (display_x as usize)
            .checked_sub(self.text_start_x)
            .and_then(|x| f.view.display_pos_to_point(display_y as usize, x))
        {
            Some(pos) => pos,
            None => return HandledEvent::Ignored,
        };

        match (modifiers, kind) {
            (NONE, MouseEventKind::Down(MouseButton::Left)) => {
                self.selection_start = Some(buffer_pos);
                HandledEvent::Consumed
            }
            (NONE, MouseEventKind::Drag(MouseButton::Left)) => {
                match self.selection_start {
                    Some(start) if start != buffer_pos => {
                        f.buffer
                            .set_cursors(vec![Cursor::Selection(Range::from_points(
                                start, buffer_pos,
                            ))]);
                    }
                    _ => {}
                }

                HandledEvent::Consumed
            }
            (NONE, MouseEventKind::Up(MouseButton::Left)) => {
                if matches!(self.selection_start, Some(start) if start == buffer_pos) {
                    f.buffer
                        .set_cursors(vec![Cursor::new(buffer_pos.y, buffer_pos.x)]);
                }

                self.selection_start = None;
                HandledEvent::Consumed
            }

            _ => HandledEvent::Ignored,
        }
    }
}
