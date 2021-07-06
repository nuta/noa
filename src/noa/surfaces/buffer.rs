use std::{
    cmp::{max, min},
    ops::Sub,
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    style::Color,
};
use noa_buffer::{Cursor, Point, Range};
use noa_langs::HighlightType;
use parking_lot::Mutex;

use crate::{
    editor::UserMessage,
    line_edit::LineEdit,
    minimap::{LineStatus, MiniMap, MiniMapCategory},
    surfaces::{prompt::CallbackResult, yes_no::YesNoChoice, FinderSurface, YesNoSurface},
    terminal::copy_to_clipboard,
    ui::{
        truncate_to_width, whitespaces, CanvasViewMut, Compositor, Context, Decoration,
        DisplayWidth, Event, HandledEvent, Layout, RectSize, Surface,
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum BufferSurfaceMode {
    Normal,
    Search,
}

pub struct BufferSurface {
    mode: BufferSurfaceMode,
    // `(y, x)`.
    cursor_position: (usize, usize),
    text_start_x: usize,
    selection_start: Option<Point>,
    minimap: Arc<Mutex<MiniMap>>,
    time_last_clicked: Instant,
    num_clicked: usize,
    scroll_ys: Vec<usize>,
    scroll_bar_x: usize,
    search_query: LineEdit,
    search_matches: Vec<Range>,
}

impl BufferSurface {
    pub fn new(minimap: Arc<Mutex<MiniMap>>) -> BufferSurface {
        BufferSurface {
            mode: BufferSurfaceMode::Normal,
            cursor_position: (0, 0),
            text_start_x: 0,
            selection_start: None,
            minimap,
            time_last_clicked: Instant::now().sub(Duration::from_secs(100)),
            num_clicked: 0,
            scroll_ys: Vec::new(),
            scroll_bar_x: 0,
            search_query: LineEdit::new(),
            search_matches: Vec::new(),
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

    fn handle_key_event_in_buffer(
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
            (KeyCode::Esc, NONE) => {
                self.search_query.clear();
                self.search_matches.clear();
            }
            (KeyCode::Char('/'), ALT) => {
                self.mode = BufferSurfaceMode::Search;
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

            (KeyCode::Char('w'), CTRL) => {
                let selections = f.buffer.prev_word_ranges();
                f.buffer.select_by_ranges(&selections);
                f.buffer.backspace();
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

    fn handle_key_event_in_search(
        &mut self,
        ctx: &mut Context,
        _compositor: &mut Compositor,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        // const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let mut f = ctx.editor.current_file().write();

        match (key.code, key.modifiers) {
            (KeyCode::Esc, NONE) => {
                self.mode = BufferSurfaceMode::Normal;
            }
            (KeyCode::Up, NONE) => {
                // TODO:
            }
            (KeyCode::Down, NONE) => {
                // TODO:
            }
            _ => {
                let prev_ver = self.search_query.rope().version();
                if !self.search_query.consume_key_event(key) {
                    return HandledEvent::Ignored;
                }

                const SEARCH_HIGHLIGHTS_MAX: usize = 256;
                if prev_ver != self.search_query.rope().version() {
                    // The search query has been updated.
                    let query = self.search_query.text();
                    trace!("search: highlighting \"{}\"", query);
                    if query.is_empty() {
                        self.search_matches.clear();
                    } else {
                        self.search_matches = f
                            .buffer
                            .find_all(&query, None)
                            .take(SEARCH_HIGHLIGHTS_MAX)
                            .collect();
                    }
                }
            }
        }

        HandledEvent::Consumed
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
        (Layout::Fixed { y: 0, x: 0 }, screen_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        Some(self.cursor_position)
    }

    fn render<'a>(&mut self, ctx: &mut Context, mut canvas: CanvasViewMut<'a>) {
        canvas.clear();

        let categories = [
            MiniMapCategory::Diagnosis,
            MiniMapCategory::Cursor,
            MiniMapCategory::Diff,
        ];

        let (max_lineno_width, text_width, text_start_x) = {
            let mut f = ctx.editor.current_file().write();
            let max_lineno_width = f.buffer.num_lines().display_width() + 1;
            let text_start_x = max_lineno_width + 2;
            let text_width = canvas.width() - (text_start_x + 1);

            f.layout_view(0, canvas.height(), text_width);
            f.highlight_from_tree_sitter();
            f.view.set_search_highlights(&self.search_matches);

            (max_lineno_width, text_width, text_start_x)
        };

        let f = ctx.editor.current_file().read();

        // Update the minimap.
        let mut minimap = self.minimap.lock();
        minimap.clear(MiniMapCategory::Cursor);
        for c in f.buffer.cursors() {
            if c == f.buffer.main_cursor() {
                continue;
            }

            minimap.insert(
                MiniMapCategory::Cursor,
                c.front().y..(c.back().y + 1),
                LineStatus::Cursor,
            );
        }

        let draw_minimap_char =
            |canvas: &mut CanvasViewMut<'a>, y: usize, x: usize, status: &LineStatus| {
                let style = match status {
                    LineStatus::Cursor => &ctx.theme.line_status_cursor,
                    LineStatus::Warning => &ctx.theme.line_status_warning,
                    LineStatus::Error => &ctx.theme.line_status_error,
                    LineStatus::AddedLine => &ctx.theme.line_status_added,
                    LineStatus::RemovedLine => &ctx.theme.line_status_removed,
                    LineStatus::ModifiedLine => &ctx.theme.line_status_modified,
                };

                canvas.draw_char(y, x, '\u{2590}' /* Right Half Block */);
                canvas.set_style(y, x, x + 1, &style);
            };

        let mut y_end = 0;
        let mut lines_end_xs = Vec::new();
        let display_lines = f.view.visible_display_lines();
        for (y, display_line) in display_lines.iter().enumerate() {
            // Draw the line number.
            let buffer_y = display_line.range.front().y;
            let lineno = buffer_y + 1;
            let lineno_width = lineno.display_width();
            let pad_len = max_lineno_width - lineno_width;
            canvas.draw_str(y, 0, &whitespaces(pad_len));
            canvas.draw_str(y, pad_len, &lineno.to_string());

            // Draw the minimap (left-side).
            let mut drew_line_status = false;
            for category in categories {
                if let Some(e) = minimap.get_containing(category, buffer_y) {
                    draw_minimap_char(&mut canvas, y, max_lineno_width, &e.value);
                    drew_line_status = true;
                }
            }

            if !drew_line_status {
                canvas.draw_char(
                    y,
                    max_lineno_width,
                    '\u{2502}', /* "Box Drawing Light Veritical" */
                );
            }

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
            let highlights_set = [
                &display_line.syntax_highlights,
                &display_line.search_highlights,
            ];

            trace!("h: {:?}", &display_line.search_highlights);
            for hs in highlights_set {
                for h in hs {
                    let x_start = text_start_x + h.range.start;
                    let x_end = text_start_x + h.range.end;
                    let (fg, bg) = match h.highlight_type {
                        HighlightType::Ident => (Some(Color::Magenta), None),
                        HighlightType::StringLiteral => (Some(Color::Green), None),
                        HighlightType::EscapeSequence => (Some(Color::Cyan), None),
                        HighlightType::PrimitiveType => (Some(Color::Cyan), None),
                        HighlightType::CMacro => (Some(Color::Magenta), None),
                        HighlightType::CIncludeArg => (Some(Color::Green), None),
                        HighlightType::MatchedBySearch => (None, Some(Color::DarkYellow)),
                    };

                    if let Some(color) = fg {
                        canvas.set_fg(y, x_start, x_end, color);
                    }
                    if let Some(color) = bg {
                        trace!("render: {:?} {}..{}", h.highlight_type, x_start, x_end);
                        canvas.set_bg(y, x_start, x_end, color);
                    }
                }
            }

            // Whitespaces after the line.
            canvas.draw_str(y, text_start_x + x, &whitespaces(text_width - x));

            lines_end_xs.push(x);
            y_end = y + 1;
        }

        // Draw the minimap (right-side).
        let num_lines = f.buffer.num_lines();
        let visible_start = display_lines.iter().next().map(|l| l.buffer_y).unwrap_or(0);
        let visible_end = display_lines
            .iter()
            .rev()
            .next()
            .map(|l| l.buffer_y)
            .unwrap_or(0);
        let visible_range = visible_start..visible_end;
        self.scroll_ys.clear();
        self.scroll_bar_x = canvas.width() - 1;
        for i in 0..canvas.height() {
            let start = (((num_lines as f64) / (canvas.height() as f64)) * (i as f64)) as usize;
            let end = (((num_lines as f64) / (canvas.height() as f64)) * ((i + 1) as f64)) as usize;
            for category in categories {
                let y_range = (start)..(end);
                if let Some(e) = minimap.iter_overlapping(category, y_range.clone()).next() {
                    draw_minimap_char(&mut canvas, i, self.scroll_bar_x, &e.value);
                }
            }

            let visible = visible_range.contains(&start);
            if visible {
                canvas.set_bg(
                    i,
                    self.scroll_bar_x,
                    self.scroll_bar_x + 1,
                    ctx.theme.line_status_visible,
                );
            }

            self.scroll_ys.push(start);
        }

        // Clear the remaining lines out of the buffer area.
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
                    canvas.set_deco(y, x, x + 1, Decoration::inverted());
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
                            canvas.set_deco(y, min(x0, x1), max(x0, x1), Decoration::inverted());
                        }
                    }
                }
            }
        }

        // Bottom bar.
        let marker = if f.buffer.is_dirty() { "[+]" } else { "" };
        let marker_width = marker.display_width();
        let colno = f.buffer.main_cursor_pos().x;
        let colno_width = colno.display_width();
        let num_cursors = f.buffer.cursors().len();
        let num_cursors_width = if num_cursors == 1 {
            0
        } else {
            3 + num_cursors.display_width()
        };
        let name_max_len = canvas
            .width()
            .saturating_sub(marker_width + 1 + 1 + colno_width + num_cursors_width);

        // Bottom bar: draw the first line.
        let bottom_bar_y0 = canvas.height() - 2;
        canvas.draw_str(bottom_bar_y0, 0, marker);
        canvas.draw_str(
            bottom_bar_y0,
            marker_width + 1,
            truncate_to_width(f.buffer.name(), name_max_len),
        );
        canvas.draw_str(
            bottom_bar_y0,
            canvas.width() - num_cursors_width - colno_width,
            &format!("{}", colno),
        );

        if num_cursors_width > 0 {
            canvas.draw_str(
                bottom_bar_y0,
                canvas.width() - num_cursors_width,
                &format!(" ({})", num_cursors),
            );
        }

        canvas.set_style(bottom_bar_y0, 0, canvas.width(), &ctx.theme.bottom_bar_text);

        // Bottom bar: draw the second line.
        let bottom_bar_y1 = canvas.height() - 1;
        let query = self.search_query.text();
        let query_width = query.display_width();
        let log_max_width = canvas.width().saturating_sub(query_width);
        canvas.draw_str(bottom_bar_y1, 0, &query);

        if let Some(message) = ctx.editor.last_message() {
            let (color, text) = match &message {
                UserMessage::Info(text) => (Color::Cyan, text),
                UserMessage::Error(text) => (Color::Red, text),
            };

            let text = truncate_to_width(text, log_max_width);
            let text_width = text.display_width();
            let x = canvas.width() - text_width;
            canvas.draw_str(bottom_bar_y1, x, text);
            canvas.set_fg(bottom_bar_y1, x, x + text_width, color);
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
        match self.mode {
            BufferSurfaceMode::Normal => self.handle_key_event_in_buffer(ctx, compositor, key),
            BufferSurfaceMode::Search => self.handle_key_event_in_search(ctx, compositor, key),
        }
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
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const NONE: KeyModifiers = KeyModifiers::NONE;

        let mut f = ctx.editor.current_file().write();

        let MouseEvent {
            kind,
            column: display_x,
            row: display_y,
            modifiers,
        } = ev;

        // Clicking the scroll bar.
        if display_x as usize == self.scroll_bar_x {
            if let Some(&buffer_y) = self.scroll_ys.get(display_y as usize) {
                match (modifiers, kind) {
                    (NONE, MouseEventKind::Down(MouseButton::Left)) => {
                        let goto = Point::new(min(buffer_y, f.buffer.num_lines()), 0);
                        f.buffer.move_cursor_to(goto);
                    }
                    _ => {}
                }
            }

            return HandledEvent::Consumed;
        }

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
            (CTRL, MouseEventKind::Down(MouseButton::Left)) => {
                let sync = ctx.editor.sync().clone();
                let file = ctx.editor.current_file().clone();
                let event_tx = ctx.event_tx.clone();
                tokio::spawn(async move {
                    match sync.lock().await.call_goto_definition(&file).await {
                        Ok(locs) => {
                            trace!("goto_definition: {:?}", locs);
                            if !locs.is_empty() {
                                event_tx.send(Event::OpenFile(locs[0].clone())).ok();
                            }
                        }
                        Err(err) => {
                            error!("goto_definition failed: {:?}", err);
                        }
                    }
                });
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
            // Triple click.
            (NONE, MouseEventKind::Up(MouseButton::Left))
                if self.num_clicked == 2
                    && self.time_last_clicked.elapsed() < Duration::from_millis(300) =>
            {
                // Select a line.
                let current_line = f.buffer.current_line_range();
                f.buffer.select_by_ranges(&[current_line]);

                self.time_last_clicked = Instant::now();
                self.num_clicked += 1;
                HandledEvent::Consumed
            }
            // Double click.
            (NONE, MouseEventKind::Up(MouseButton::Left))
                if self.num_clicked == 1
                    && self.time_last_clicked.elapsed() < Duration::from_millis(300) =>
            {
                // Select a word.
                if let Some(current_word) = f.buffer.current_word_range() {
                    f.buffer.select_by_ranges(&[current_word]);
                }
                self.time_last_clicked = Instant::now();
                self.num_clicked += 1;
                HandledEvent::Consumed
            }
            // Single click.
            (NONE, MouseEventKind::Up(MouseButton::Left)) => {
                // Move cursor.
                if matches!(self.selection_start, Some(start) if start == buffer_pos) {
                    f.buffer
                        .set_cursors(vec![Cursor::new(buffer_pos.y, buffer_pos.x)]);
                }

                self.time_last_clicked = Instant::now();
                self.num_clicked = 1;
                self.selection_start = None;
                HandledEvent::Consumed
            }

            _ => HandledEvent::Ignored,
        }
    }
}
