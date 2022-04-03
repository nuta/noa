use std::{
    cmp::{max, min},
    sync::Arc,
    time::{Duration, Instant},
};

use noa_buffer::{
    cursor::{Position, Range},
    display_width::DisplayWidth,
};
use noa_common::{debug_warn, logger::OopsExt};
use noa_compositor::{
    canvas::{CanvasViewMut, Color, Style},
    surface::{HandledEvent, KeyEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    Compositor,
};

use tokio::sync::{mpsc::UnboundedSender, Notify};

use crate::{
    actions::execute_action_or_notify,
    completion::{clear_completion, complete},
    config::{get_keybinding_for, theme_for, KeyBindingScope},
    editor::Editor,
    linemap::LineStatus,
};

use super::{completion_view::CompletionView, meta_line_view::META_LINE_HEIGHT};

pub struct BufferView {
    quit_tx: UnboundedSender<()>,
    render_request: Arc<Notify>,
    /// The cursor position in surface-local `(y, x)`.
    cursor_position: (usize, usize),
    selection_start: Option<Position>,
    time_last_clicked: Instant,
    num_clicked: usize,
    buffer_x: usize,
}

impl BufferView {
    pub fn new(quit_tx: UnboundedSender<()>, render_request: Arc<Notify>) -> BufferView {
        BufferView {
            quit_tx,
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

    pub fn post_update_job(&mut self, editor: &mut Editor) {
        let doc = editor.documents.current_mut();
        doc.post_update_job(editor.repo.as_ref(), &editor.render_request);
    }

    pub fn show_completion(&mut self, editor: &mut Editor) {
        let doc = editor.documents.current_mut();
        if doc.buffer().cursors().len() != 1 {
            return;
        }

        let doc_id = doc.id();
        let buffer = doc.raw_buffer().clone();
        let lang = doc.buffer().language();
        let path = doc.path().to_owned();
        let main_cursor = doc.buffer().main_cursor().clone();
        let words = editor.documents.words();
        editor.jobs.await_in_mainloop(
            complete(buffer, lang, path, main_cursor, words),
            move |editor, compositor, items| {
                if let Some(items) = items {
                    editor
                        .documents
                        .get_mut_document_by_id(doc_id)
                        .unwrap()
                        .set_completion_items(items);

                    let view: &mut CompletionView =
                        compositor.get_mut_surface_by_name("completion");
                    view.set_active(true);
                }
            },
        )
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

    fn layout(&mut self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        (
            Layout::Fixed { y: 0, x: 0 },
            RectSize {
                width: screen_size.width,
                height: screen_size.height.saturating_sub(META_LINE_HEIGHT),
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
            buffer_width = canvas.width() - buffer_x - 1 /* row_end_marker */;
            buffer_height = canvas.height();

            doc.layout_view(&editor.find_query.text(), buffer_height, buffer_width);
        }

        self.buffer_x = buffer_x;

        let doc = editor.documents.current();
        let buffer = doc.buffer();
        let main_cursor = buffer.main_cursor();
        let linemap = doc.linemap().load();

        // Buffer contents.
        let main_cursor_pos = main_cursor.moving_position();
        let scroll_x = doc.view().scroll_x();
        for (i_y, row) in doc.view().visible_rows().iter().enumerate() {
            let canvas_y = buffer_y + i_y;

            // Highlight the current line.
            if main_cursor.selection().is_empty() && main_cursor_pos.y == row.lineno - 1 {
                canvas.apply_style(
                    canvas_y,
                    buffer_x,
                    canvas.width() - 1,
                    theme_for("buffer.current_line"),
                );
            }

            // Draw line status.
            if let Some(status) = linemap.get(row.lineno - 1) {
                let theme_key = if status & LineStatus::REPO_DIFF_MASK == LineStatus::MODIFIED {
                    Some("line_status.modified")
                } else if status & LineStatus::REPO_DIFF_MASK == LineStatus::ADDED {
                    Some("line_status.added")
                } else if status & LineStatus::REPO_DIFF_MASK == LineStatus::REMOVED {
                    Some("line_status.deleted")
                } else {
                    debug_warn!("ignored line status: {:?}", status);
                    None
                };

                if let Some(theme_key) = theme_key {
                    canvas.write_char_with_style(canvas_y, 0, ' ', theme_for(theme_key));
                }
            }

            // Draw lineno.
            let lineno_x = lineno_x + max_lineno_width - row.lineno.display_width();
            canvas.write_str(canvas_y, lineno_x, &format!("{}", row.lineno));

            // Draw each characters in the row.
            let mut row_end_marker = None;
            let mut canvas_x = buffer_x;
            for (grapheme, pos) in row
                .graphemes
                .iter()
                .skip(scroll_x)
                .zip(row.positions.iter().skip(scroll_x))
            {
                if canvas_x - buffer_x >= buffer_width {
                    // The cursor may go beyond the right edge of the screen if
                    // soft wrapping is disabled.
                    continue;
                }

                // Draw the character.
                canvas.write(canvas_y, canvas_x, *grapheme);

                // Check if the main cursor is at this position.
                if *pos == main_cursor_pos {
                    self.cursor_position = (canvas_y, canvas_x);
                }

                // Update decoration if the cursor includes or is located at
                // this position.
                for (_i, c) in buffer.cursors().iter().enumerate() {
                    if c.selection().contains(*pos)
                        || (!c.is_main_cursor() && c.position() == Some(*pos))
                    {
                        canvas.set_inverted(canvas_y, canvas_x, canvas_x + grapheme.width, true);

                        let mut next_pos = *pos;
                        next_pos.move_by(buffer, 0, 0, 0, 1);
                        if c.selection().contains(next_pos) {
                            row_end_marker = Some((' ', canvas_x + grapheme.width));
                        }

                        break;
                    }
                }

                canvas_x += grapheme.width;
            }

            // Cursors at a empty row.
            if row.is_empty()
                && buffer.cursors().iter().enumerate().any(|(_i, c)| {
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
                    .any(|(_i, c)| !c.is_main_cursor() && c.position() == Some(end_of_row_pos))
                {
                    row_end_marker = Some((' ', buffer_x + row.total_width()));
                }
            }

            if let Some((_ch, x)) = row_end_marker {
                canvas.set_inverted(canvas_y, x, x + 1, true);
            }

            // The main cursor is at the end of line.
            if main_cursor_pos.y == row.lineno - 1 && main_cursor_pos.x == row.last_position().x + 1
            {
                self.cursor_position = (canvas_y, buffer_x + row.total_width());
            // The main cursor is at the end of empty line.
            } else if main_cursor_pos.y == row.lineno - 1 && row.is_empty() {
                self.cursor_position = (canvas_y, buffer_x);
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
        editor: &mut Editor,
        compositor: &mut Compositor<Self::Context>,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.documents.current_mut();
        let prev_rope = doc.raw_buffer().rope().clone();

        clear_completion(compositor, doc);

        let mut show_completion = false;
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.quit_tx.send(()).oops();
            }
            (KeyCode::Esc, NONE) => {
                doc.buffer_mut().clear_secondary_cursors();
            }
            (KeyCode::Backspace, NONE) => {
                doc.buffer_mut().backspace();
            }
            (KeyCode::Delete, _) => {
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
            (KeyCode::Enter, NONE) => {
                doc.buffer_mut().insert_newline_and_indent();
            }
            (KeyCode::Tab, NONE) => {
                doc.buffer_mut().indent();
            }
            (KeyCode::BackTab, _) => {
                doc.buffer_mut().deindent();
            }
            (KeyCode::Char(ch), NONE) | (KeyCode::Char(ch), SHIFT) => {
                doc.buffer_mut().smart_insert_char(ch);
                show_completion = true;
            }
            _ => {
                if let Some(binding) =
                    get_keybinding_for(KeyBindingScope::Buffer, key.code, key.modifiers)
                {
                    execute_action_or_notify(editor, compositor, &binding.action);
                } else {
                    trace!("unhandled key = {:?}", key);
                }
            }
        }

        let current_rope = editor
            .documents
            .current()
            .buffer()
            .raw_buffer()
            .rope()
            .clone();

        if prev_rope != current_rope {
            self.post_update_job(editor);
            if show_completion {
                self.show_completion(editor);
            }
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        editor: &mut Editor,
        compositor: &mut Compositor<Editor>,
        s: &str,
    ) -> HandledEvent {
        let doc = editor.documents.current_mut();
        clear_completion(compositor, doc);
        doc.buffer_mut().insert(s);
        self.post_update_job(editor);
        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        editor: &mut Editor,
        compositor: &mut Compositor<Self::Context>,
        kind: MouseEventKind,
        modifiers: KeyModifiers,
        surface_y: usize,
        surface_x: usize,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const _CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.documents.current_mut();

        clear_completion(compositor, doc);

        if kind == MouseEventKind::ScrollDown {
            doc.movement().scroll_down();
            return HandledEvent::Consumed;
        }

        if kind == MouseEventKind::ScrollUp {
            doc.movement().scroll_up();
            return HandledEvent::Consumed;
        }

        if surface_x >= self.buffer_x {
            if let Some(clicked_pos) = doc
                .view()
                .get_position_from_screen_yx(surface_y, surface_x - self.buffer_x)
            {
                match (kind, modifiers) {
                    (MouseEventKind::Down(MouseButton::Left), _) => {
                        self.selection_start = Some(clicked_pos);
                    }
                    // Single click.
                    (MouseEventKind::Up(MouseButton::Left), NONE)
                        if self.time_last_clicked.elapsed() > Duration::from_millis(400) =>
                    {
                        // Move cursor.
                        if matches!(self.selection_start, Some(start) if start == clicked_pos) {
                            doc.buffer_mut().move_main_cursor_to_pos(clicked_pos);
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
                        doc.buffer_mut().select_whole_line(clicked_pos);
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
                        Some(start) if start != clicked_pos => {
                            doc.buffer_mut().select_main_cursor(
                                start.y,
                                start.x,
                                clicked_pos.y,
                                clicked_pos.x,
                            );
                        }
                        _ => {}
                    },
                    // Single click + Shift.
                    (MouseEventKind::Up(MouseButton::Left), SHIFT)
                        if self.time_last_clicked.elapsed() > Duration::from_millis(400) =>
                    {
                        // Select until the clicked position.
                        doc.buffer_mut().deselect_cursors();
                        let from = doc.buffer().main_cursor().moving_position();
                        doc.buffer_mut().select_main_cursor(
                            from.y,
                            from.x,
                            clicked_pos.y,
                            clicked_pos.x,
                        );

                        self.time_last_clicked = Instant::now();
                        self.num_clicked = 1;
                        self.selection_start = None;
                    }
                    // Single click + Alt.
                    (MouseEventKind::Up(MouseButton::Left), ALT)
                        if self.time_last_clicked.elapsed() > Duration::from_millis(400) =>
                    {
                        // Add a cursor.
                        if matches!(self.selection_start, Some(start) if start == clicked_pos) {
                            doc.buffer_mut()
                                .add_cursor(Range::from_single_position(clicked_pos));
                        }

                        trace!("Single click + Alt");
                        self.time_last_clicked = Instant::now();
                        self.num_clicked = 1;
                        self.selection_start = None;
                    }
                    // Single click + Alt + Shift.
                    (MouseEventKind::Up(MouseButton::Left), modifiers)
                        if modifiers == ALT | SHIFT
                            && self.time_last_clicked.elapsed() > Duration::from_millis(400) =>
                    {
                        // Add a cursor until the clicked position.
                        let buffer = doc.buffer_mut();
                        buffer.deselect_cursors();
                        let main_cursor = buffer.main_cursor().selection();
                        let main_pos = main_cursor.front();
                        let n = if main_cursor.front().y == main_cursor.back().y {
                            main_cursor.back().x - main_cursor.front().x
                        } else {
                            0
                        };

                        for y in min(main_pos.y, clicked_pos.y)..=max(main_pos.y, clicked_pos.y) {
                            let x = main_pos.x;
                            let line_len = buffer.line_len(y);
                            buffer.add_cursor(Range::new(
                                y,
                                min(x, line_len),
                                y,
                                min(x + n, line_len),
                            ));
                        }

                        self.time_last_clicked = Instant::now();
                        self.num_clicked = 1;
                        self.selection_start = None;
                    }
                    _ => {
                        return HandledEvent::Ignored;
                    }
                }
            }
        }

        HandledEvent::Consumed
    }
}
