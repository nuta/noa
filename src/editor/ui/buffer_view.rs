use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use noa_buffer::{
    cursor::{Position, Range},
    display_width::DisplayWidth,
};
use noa_common::oops::OopsExt;
use noa_compositor::{
    canvas::{CanvasViewMut, Decoration},
    surface::{HandledEvent, KeyEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    Compositor,
};
use noa_proxy::lsp_types::HoverContents;
use tokio::sync::{mpsc::UnboundedSender, oneshot, Notify};

use crate::{
    actions::{execute_action, execute_action_or_notify},
    clipboard::{ClipboardData, SystemClipboardData},
    completion::{clear_completion, complete},
    editor::Editor,
    keybindings::get_keybinding_for,
    linemap::LineStatus,
    theme::theme_for,
    ui::finder_view::FinderView,
    ui::markdown::Markdown,
};

use super::completion_view::CompletionView;

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
        doc.post_update_job();

        // Completion.
        if doc.buffer().cursors().len() == 1 {
            let doc_id = doc.id();
            let proxy = editor.proxy.clone();
            let buffer = doc.raw_buffer().clone();
            let lang = doc.buffer().language();
            let path = doc.path().to_owned();
            let main_cursor = doc.buffer().main_cursor().clone();
            let words = editor.documents.words();
            editor.await_in_mainloop(
                async move {
                    let items = complete(proxy, buffer, lang, path, main_cursor, words).await;
                    Ok(items)
                },
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
                height: screen_size.height.saturating_sub(1),
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
        let linemap = doc.linemap().load();

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
                    warn!("ignored line status: {:?}", status);
                    None
                };

                if let Some(theme_key) = theme_key {
                    canvas.write_char_with_style(y, 0, ' ', theme_for(theme_key));
                }
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
                for (_i, c) in buffer.cursors().iter().enumerate() {
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
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.documents.current_mut();
        let prev_rope = doc.raw_buffer().rope().clone();

        clear_completion(doc, compositor);

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
                doc.buffer_mut().insert_char(ch);
            }
            _ => {
                if let Some(binding) = get_keybinding_for("buffer", key.code, key.modifiers) {
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
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        compositor: &mut Compositor<Editor>,
        editor: &mut Editor,
        s: &str,
    ) -> HandledEvent {
        let doc = editor.documents.current_mut();
        clear_completion(doc, compositor);
        doc.buffer_mut().insert(s);
        self.post_update_job(editor);
        HandledEvent::Consumed
    }

    fn handle_mouse_event(
        &mut self,
        compositor: &mut Compositor<Self::Context>,
        editor: &mut Editor,
        kind: MouseEventKind,
        modifiers: KeyModifiers,
        surface_y: usize,
        surface_x: usize,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const _CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const _SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.documents.current_mut();

        clear_completion(doc, compositor);

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
                    (MouseEventKind::Down(MouseButton::Left), _) => {
                        self.selection_start = Some(pos);
                    }
                    // Single click.
                    (MouseEventKind::Up(MouseButton::Left), NONE)
                        if self.time_last_clicked.elapsed() > Duration::from_millis(400) =>
                    {
                        // Move cursor.
                        if matches!(self.selection_start, Some(start) if start == pos) {
                            doc.buffer_mut().move_main_cursor_to_pos(pos);
                        }

                        // Hover.
                        let proxy = editor.proxy.clone();
                        let lang = doc.buffer().language();
                        let path = doc.path().to_owned();
                        let pos = doc.buffer().main_cursor().moving_position().into();
                        tokio::spawn(async move {
                            match proxy.hover(lang, &path, pos).await {
                                Ok(Some(hover)) => match hover {
                                    HoverContents::Scalar(text) => {
                                        notify_info!("{}", Markdown::from(text));
                                    }
                                    HoverContents::Array(items) if !items.is_empty() => {
                                        notify_info!("{}", Markdown::from(items[0].clone()));
                                    }
                                    HoverContents::Markup(markup) => {
                                        notify_info!("{}", Markdown::from(markup));
                                    }
                                    _ => {
                                        warn!("unsupported hover type: {:?}", hover);
                                    }
                                },
                                Ok(None) => {
                                    notify_warn!("no hover info");
                                }
                                Err(err) => {
                                    notify_error!("failed to get hover info: {}", err);
                                }
                            }
                        });

                        trace!("Single click");
                        self.time_last_clicked = Instant::now();
                        self.num_clicked = 1;
                        self.selection_start = None;
                    }
                    // Single click + Alt.
                    (MouseEventKind::Up(MouseButton::Left), ALT)
                        if self.time_last_clicked.elapsed() > Duration::from_millis(400) =>
                    {
                        // Add a cursor.
                        if matches!(self.selection_start, Some(start) if start == pos) {
                            doc.buffer_mut()
                                .add_cursor(Range::from_single_position(pos));
                        }

                        trace!("Single click + Alt");
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
                                .select_main_cursor(start.y, start.x, pos.y, pos.x);
                        }
                        _ => {}
                    },
                    // Dragging + Alt
                    (MouseEventKind::Drag(MouseButton::Left), ALT) => match self.selection_start {
                        Some(start) if start != pos => {
                            doc.buffer_mut()
                                .add_cursor(Range::from_positions(start, pos));
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
