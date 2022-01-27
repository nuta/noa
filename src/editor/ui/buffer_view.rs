use noa_buffer::{cursor::Position, display_width::DisplayWidth};
use noa_compositor::{
    canvas::{CanvasViewMut, Decoration, Style},
    surface::{HandledEvent, KeyEvent, Layout, MouseEvent, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers},
};
use tokio::{sync::oneshot, task};

use crate::{
    clipboard::{ClipboardData, SystemClipboardData},
    editor::Editor,
};

pub struct BufferView {
    quit_tx: Option<oneshot::Sender<()>>,
    /// The cursor position in surface-local `(y, x)`.
    cursor_position: (usize, usize),
}

impl BufferView {
    pub fn new(quit_tx: oneshot::Sender<()>) -> BufferView {
        BufferView {
            quit_tx: Some(quit_tx),
            cursor_position: (0, 0),
        }
    }
}

impl Surface for BufferView {
    type Context = Editor;

    fn name(&self) -> &str {
        "buffer"
    }

    fn is_visible(&self, _editor: &mut Editor) -> bool {
        true
    }

    fn layout(&self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        (
            Layout::Fixed { y: 0, x: 0 },
            RectSize {
                height: screen_size.height.saturating_sub(2 /* bottom line */),
                width: screen_size.width,
            },
        )
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        Some(self.cursor_position)
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        let lineno_x;
        let max_lineno_width;
        let buffer_y;
        let buffer_x;
        let buffer_width;
        let buffer_height;
        {
            let doc = editor.documents.current_mut();
            let buffer = doc.buffer();
            lineno_x = 1;
            max_lineno_width = buffer.num_lines().display_width();
            buffer_y = 0;
            buffer_x = lineno_x + max_lineno_width + 1 /* line status */;
            buffer_width = canvas.width() - buffer_x  - 2 /* row_end_marker and mini map */;
            buffer_height = canvas.height() - 2 /* bottom line */;

            doc.layout_view(buffer_width);
        }

        let doc = editor.documents.current();
        let buffer = doc.buffer();

        // Buffer contents.
        let main_cursor_pos = buffer.main_cursor().selection().start;
        for (i_y, (row)) in doc.view().display_rows().enumerate() {
            let y = buffer_y + i_y;

            // Draw lineno.
            let lineno_x = lineno_x + max_lineno_width - row.lineno.display_width();
            canvas.write_str(y, lineno_x, &format!("{}", row.lineno));

            // Draw each characters in the row.
            let mut row_end_marker = None;
            for (i_x, (grapheme, pos)) in row.graphemes.iter().zip(row.positions.iter()).enumerate()
            {
                let x = buffer_x + i_x;
                if x >= canvas.width() {
                    // The cursor may go beyond the right edge of the screen if
                    // soft wrapping is disabled.
                    continue;
                }

                // Draw the character.
                canvas.write(y, x, *grapheme);

                // Check if the main cursor is at this position.
                if *pos == main_cursor_pos {
                    self.cursor_position = (y, x);
                }

                // Update decoration if the cursor includes or is located at
                // this position.
                for (i, c) in buffer.cursors().iter().enumerate() {
                    if c.selection().contains(*pos) || (i != 0 && c.position() == Some(*pos)) {
                        canvas.set_decoration(y, x, x + 1, Decoration::inverted());

                        let mut next_pos = *pos;
                        next_pos.move_by(buffer, 0, 0, 0, 1);
                        if c.selection().contains(next_pos) {
                            row_end_marker = Some((' ', x + 1));
                        }

                        break;
                    }
                }
            }

            if row.is_empty()
                && buffer
                    .cursors()
                    .iter()
                    .any(|c| c.selection().contains(Position::new(row.lineno - 1, 0)))
            {
                row_end_marker = Some((' ', buffer_x));
            }

            if let Some((ch, x)) = row_end_marker {
                canvas.set_decoration(y, x, x + 1, Decoration::inverted());
            }

            // The main cursor is at the end of line.
            if main_cursor_pos.y == row.lineno - 1 && main_cursor_pos.x == row.graphemes.len() {
                self.cursor_position = (y, buffer_x + row.graphemes.len());
            }
        }
    }

    fn handle_key_event(&mut self, editor: &mut Editor, key: KeyEvent) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let mut notifications = &mut editor.notifications;
        let mut doc = editor.documents.current_mut();

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.quit_tx.take().unwrap().send(());
            }
            (KeyCode::Char('s'), CTRL) => {
                notifications.maybe_error(doc.save_to_file());
            }
            (KeyCode::Char('u'), CTRL) => {
                doc.buffer_mut().undo();
            }
            (KeyCode::Char('y'), CTRL) => {
                doc.buffer_mut().redo();
            }
            (KeyCode::Char('c'), CTRL) => {
                editor
                    .clipboard
                    .copy_into_clipboard(ClipboardData::from_buffer(doc.buffer()));
            }
            (KeyCode::Char('x'), CTRL) => {
                let buffer = doc.buffer_mut();
                match editor.clipboard.copy_from_clipboard() {
                    Ok(SystemClipboardData::Ours(ClipboardData { texts })) => {
                        let strs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                        buffer.insert_multiple(&strs);
                    }
                    Ok(SystemClipboardData::Others(string)) => {
                        buffer.insert(&string);
                    }
                    Err(err) => {
                        error!("failed to copy from clipboard: {:?}", err);
                    }
                }
            }
            (KeyCode::Char('k'), CTRL) => {
                doc.buffer_mut().truncate();
            }
            (KeyCode::Char('a'), CTRL) => {
                doc.buffer_mut().move_to_beginning_of_line();
            }
            (KeyCode::Char('e'), CTRL) => {
                doc.buffer_mut().move_to_end_of_line();
            }
            (KeyCode::Char('f'), ALT) => {
                doc.buffer_mut().move_to_next_word();
            }
            (KeyCode::Char('b'), ALT) => {
                doc.buffer_mut().move_to_prev_word();
            }
            (KeyCode::Up, ALT) => {
                doc.buffer_mut().move_lines_up();
            }
            (KeyCode::Down, ALT) => {
                doc.buffer_mut().move_lines_down();
            }
            (KeyCode::Up, modifiers) if modifiers == (CTRL | ALT) => {
                doc.movement().add_cursors_up();
            }
            (KeyCode::Down, modifiers) if modifiers == (CTRL | ALT) => {
                doc.movement().add_cursors_down();
            }
            (KeyCode::Up, modifiers) if modifiers == (SHIFT | ALT) => {
                doc.buffer_mut().duplicate_lines_up();
            }
            (KeyCode::Down, modifiers) if modifiers == (SHIFT | ALT) => {
                doc.buffer_mut().duplicate_lines_down();
            }
            (KeyCode::Char('w'), CTRL) => {
                doc.buffer_mut().delete_current_word();
            }
            (KeyCode::Backspace, NONE) => {
                doc.buffer_mut().backspace();
            }
            (KeyCode::Char('d'), CTRL) | (KeyCode::Delete, _) => {
                doc.buffer_mut().delete();
            }
            (KeyCode::Up, NONE) => {
                doc.movement().move_cursors_up();
            }
            (KeyCode::Down, NONE) => {
                doc.movement().move_cursors_down();
            }
            (KeyCode::Left, NONE) => {
                doc.movement().move_cursors_left();
            }
            (KeyCode::Right, NONE) => {
                doc.movement().move_cursors_right();
            }
            (KeyCode::Left, modifiers) if modifiers == ALT => {
                doc.buffer_mut().move_to_prev_word();
            }
            (KeyCode::Right, modifiers) if modifiers == ALT => {
                doc.buffer_mut().move_to_next_word();
            }
            (KeyCode::Up, SHIFT) => {
                doc.movement().select_up();
            }
            (KeyCode::Down, SHIFT) => {
                doc.movement().select_down();
            }
            (KeyCode::Left, SHIFT) => {
                doc.movement().select_left();
            }
            (KeyCode::Right, SHIFT) => {
                doc.movement().select_right();
            }
            (KeyCode::Left, modifiers) if modifiers == (SHIFT | CTRL) => {
                doc.movement().select_until_beginning_of_line();
            }
            (KeyCode::Right, modifiers) if modifiers == (SHIFT | CTRL) => {
                doc.movement().select_until_end_of_line();
            }
            (KeyCode::Left, modifiers) if modifiers == (SHIFT | ALT) => {
                doc.buffer_mut().select_prev_word();
            }
            (KeyCode::Right, modifiers) if modifiers == (SHIFT | ALT) => {
                doc.buffer_mut().select_next_word();
            }
            (KeyCode::Enter, NONE) => {
                doc.buffer_mut().insert_newline_and_indent();
            }
            (KeyCode::Tab, NONE) => {
                doc.buffer_mut().deindent();
            }
            (KeyCode::BackTab, NONE) => {
                doc.buffer_mut().indent();
            }
            (KeyCode::Char(ch), NONE) => {
                doc.buffer_mut().insert_char(ch);
            }
            (KeyCode::Char(ch), SHIFT) => {
                doc.buffer_mut().insert_char(ch);
            }
            _ => {
                trace!("unhandled key = {:?}", key);
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(&mut self, editor: &mut Editor, s: &str) -> HandledEvent {
        editor.documents.current_mut().buffer_mut().insert(s);
        HandledEvent::Consumed
    }

    fn handle_mouse_event(&mut self, editor: &mut Editor, _ev: MouseEvent) -> HandledEvent {
        HandledEvent::Ignored
    }
}
