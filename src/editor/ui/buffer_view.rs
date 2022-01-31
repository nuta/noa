use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use noa_buffer::{
    cursor::{Cursor, Position},
    display_width::DisplayWidth,
};
use noa_common::oops::OopsExt;
use noa_compositor::{
    canvas::{CanvasViewMut, Decoration},
    surface::{HandledEvent, KeyEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    Compositor,
};
use tokio::sync::{oneshot, Notify};

use crate::{
    clipboard::{ClipboardData, SystemClipboardData},
    editor::Editor,
    theme::{theme_for, ThemeKey},
    ui::finder_view::FinderView,
};

pub struct BufferView {
    quit_tx: Option<oneshot::Sender<()>>,
    render_request: Arc<Notify>,
    /// The cursor position in surface-local `(y, x)`.
    cursor_position: (usize, usize),
    selection_start: Option<Position>,
    time_last_clicked: Instant,
    num_clicked: usize,
    buffer_x: usize,
}

impl BufferView {
    pub fn new(quit_tx: oneshot::Sender<()>, render_request: Arc<Notify>) -> BufferView {
        BufferView {
            quit_tx: Some(quit_tx),
            render_request,
            cursor_position: (0, 0),
            selection_start: None,
            time_last_clicked: Instant::now()
                .checked_sub(Duration::from_secs(10000 /* long time before */))
                .unwrap(),
            num_clicked: 0,
            buffer_x: 0,
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

    fn is_active(&self, _editor: &mut Editor) -> bool {
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
            buffer_height = canvas.height();

            doc.layout_view(buffer_height, buffer_width);
        }

        self.buffer_x = buffer_x;

        let doc = editor.documents.current();
        let buffer = doc.buffer();
        let main_cursor = buffer.main_cursor();
        let minimap = doc.minimap();

        // Buffer contents.
        let main_cursor_pos = main_cursor.moving_position();
        for (i_y, row) in doc.view().visible_rows().iter().enumerate() {
            let y = buffer_y + i_y;

            // Highlight the current line.
            if main_cursor.selection().is_empty() && main_cursor_pos.y == row.lineno - 1 {
                canvas.apply_style(
                    y,
                    buffer_x,
                    canvas.width() - 1,
                    theme_for(ThemeKey::CurrentLine),
                );
            }

            // Draw line status.
            if let Some(status) = minimap.get(row.lineno - 1) {
                canvas.write_char_with_style(y, 0, ' ', theme_for(ThemeKey::LineStatus(status)));
            }

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
                    if c.selection().contains(*pos)
                        || (!c.is_main_cursor() && c.position() == Some(*pos))
                    {
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

            // Cursors at a empty row.
            if row.is_empty()
                && buffer.cursors().iter().enumerate().any(|(i, c)| {
                    !c.is_main_cursor() && c.selection().overlaps(Position::new(row.lineno - 1, 0))
                })
            {
                row_end_marker = Some((' ', buffer_x));
            }

            // Cursors at the end of a row.
            // TODO: Merge above if block
            if !row.is_empty() {
                let last_pos = row.last_position();
                let end_of_row_pos = Position::new(last_pos.y, last_pos.x + 1);
                if buffer
                    .cursors()
                    .iter()
                    .enumerate()
                    .any(|(i, c)| !c.is_main_cursor() && c.position() == Some(end_of_row_pos))
                {
                    row_end_marker = Some((' ', buffer_x + row.len_chars()));
                }
            }

            if let Some((_ch, x)) = row_end_marker {
                canvas.set_decoration(y, x, x + 1, Decoration::inverted());
            }

            // The main cursor is at the end of line.
            if main_cursor_pos.y == row.lineno - 1 && main_cursor_pos.x == row.len_chars() {
                self.cursor_position = (y, buffer_x + row.len_chars());
            }
        }

        // Re-render to update flashing later.
        if let Some(duration) = doc.flashes().next_timeout() {
            let render_request = self.render_request.clone();
            tokio::spawn(async move {
                tokio::time::sleep(duration).await;
                render_request.notify_one();
            });
        }
    }

    fn handle_key_event(
        &mut self,
        compositor: &mut Compositor<Self::Context>,
        editor: &mut Editor,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.documents.current_mut();
        let prev_rope = doc.buffer().raw_buffer().rope().clone();

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.quit_tx.take().unwrap().send(()).oops();
            }
            (KeyCode::Esc, NONE) => {
                doc.buffer_mut().clear_multiple_cursors();
            }
            (KeyCode::Char('f'), CTRL) => {
                compositor
                    .get_mut_surface_by_name::<FinderView>("finder")
                    .set_active(true);
            }
            (KeyCode::Char('s'), CTRL) => {
                if let Err(err) = doc.save_to_file() {
                    notify_anyhow_error!(err);
                }
            }
            (KeyCode::Char('u'), CTRL) => {
                doc.buffer_mut().undo();
            }
            (KeyCode::Char('y'), CTRL) => {
                doc.buffer_mut().redo();
            }
            (KeyCode::Char('c'), CTRL) => {
                if let Err(err) = editor
                    .clipboard
                    .copy_into_clipboard(ClipboardData::from_buffer(doc.buffer()))
                {
                    notify_warn!("failed to copy to clipboard: {}", err);
                }
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

        let current_rope = doc.buffer().raw_buffer().rope().clone();
        if prev_rope != current_rope {
            doc.post_update_job(&editor.repo, &editor.render_request);
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor<Editor>,
        editor: &mut Editor,
        s: &str,
    ) -> HandledEvent {
        let doc = editor.documents.current_mut();
        doc.buffer_mut().insert(s);
        doc.post_update_job(&editor.repo, &editor.render_request);
        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        editor: &mut Editor,
        kind: MouseEventKind,
        modifiers: KeyModifiers,
        surface_y: usize,
        surface_x: usize,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.documents.current_mut();

        if kind == MouseEventKind::ScrollDown {
            doc.movement().scroll_down();
            return HandledEvent::Consumed;
        }

        if kind == MouseEventKind::ScrollUp {
            doc.movement().scroll_up();
            return HandledEvent::Consumed;
        }

        if surface_x >= self.buffer_x {
            if let Some(pos) = doc
                .view()
                .get_position_from_yx(surface_y, surface_x - self.buffer_x)
            {
                match (kind, modifiers) {
                    (MouseEventKind::Down(MouseButton::Left), NONE) => {
                        self.selection_start = Some(pos);
                    }
                    // Single click.
                    (MouseEventKind::Up(MouseButton::Left), NONE)
                        if self.time_last_clicked.elapsed() > Duration::from_millis(400) =>
                    {
                        // Move cursor.
                        if matches!(self.selection_start, Some(start) if start == pos) {
                            doc.buffer_mut().set_cursors(&[Cursor::new(pos.y, pos.x)]);
                        }

                        trace!("Single click");
                        self.time_last_clicked = Instant::now();
                        self.num_clicked = 1;
                        self.selection_start = None;
                    }
                    // Double click.
                    (MouseEventKind::Up(MouseButton::Left), NONE) if self.num_clicked == 1 => {
                        trace!("Double click");
                        doc.buffer_mut().select_current_word();
                        self.time_last_clicked = Instant::now();
                        self.num_clicked += 1;
                    }
                    // Triple click.
                    (MouseEventKind::Up(MouseButton::Left), NONE) if self.num_clicked == 2 => {
                        trace!("Triple click");
                        doc.buffer_mut().select_whole_line(pos);
                        self.time_last_clicked = Instant::now();
                        self.num_clicked += 1;
                    }
                    // 4-times click.
                    (MouseEventKind::Up(MouseButton::Left), NONE) if self.num_clicked == 3 => {
                        trace!("4-times click");
                        doc.buffer_mut().select_whole_buffer();
                        self.time_last_clicked = Instant::now();
                        self.num_clicked += 1;
                    }
                    // Dragging
                    (MouseEventKind::Drag(MouseButton::Left), NONE) => match self.selection_start {
                        Some(start) if start != pos => {
                            doc.buffer_mut()
                                .select_main_cursor_yx(start.y, start.x, pos.y, pos.x);
                        }
                        _ => {}
                    },
                    _ => {
                        return HandledEvent::Ignored;
                    }
                }
            }
        }

        HandledEvent::Consumed
    }
}
