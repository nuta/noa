use std::{
    cmp::{max, min},
    process::Stdio,
    time::Duration,
};

use noa_buffer::{
    cursor::Position,
    display_width::DisplayWidth,
    paragraph_iter::Paragraph,
    reflow_iter::{PrintableGrapheme, ReflowItem},
};
use noa_compositor::{
    canvas::{CanvasViewMut, Grapheme},
    compositor::Compositor,
    surface::{HandledEvent, KeyEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers},
};
use tokio::{
    sync::mpsc::{self, UnboundedSender},
    time,
};

use crate::{
    actions::execute_action_or_notify,
    config::{get_keybinding_for, theme_for, KeyBindingScope},
    editor::Editor,
    notification::{notification_manager, Notification},
    notify_error,
};

enum MainloopCommand {
    Quit,
    ExternalCommand(std::process::Command),
}

pub struct Ui {
    compositor: Compositor<Editor>,
    editor: Editor,
}

impl Ui {
    pub fn new(editor: Editor) -> Self {
        Ui {
            compositor: Compositor::new(),
            editor,
        }
    }

    pub async fn run(mut self) {
        info!("restarting compositor");
        let (mainloop_tx, mut mainloop_rx) = mpsc::unbounded_channel();
        self.compositor
            .add_frontmost_layer(Box::new(Text::new(mainloop_tx.clone())));
        self.compositor
            .add_frontmost_layer(Box::new(MetaLine::new()));
        'outer: loop {
            trace_timing!("render", 5 /* ms */, {
                self.compositor.render(&mut self.editor);
            });

            let timeout = time::sleep(Duration::from_millis(3));
            tokio::pin!(timeout);

            // Handle all pending events until the timeout is reached.
            'inner: for i in 0.. {
                tokio::select! {
                    biased;

                    Some(command) = mainloop_rx.recv() => {
                        match command {
                            MainloopCommand::Quit => break 'outer,
                            MainloopCommand::ExternalCommand(mut cmd) => {
                                cmd.stdin(Stdio::inherit())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::inherit());

                                let result = self.compositor.run_in_cooked_mode(&mut self.editor, || {
                                    cmd.spawn().and_then(|child| child.wait_with_output())
                                }).await;

                                match result {
                                    Ok(output) => {
                                        info!("output: {:?}", output);
                                    }
                                    Err(err) => notify_error!("failed to spawn: {}", err),
                                }
                            }
                        }
                    }

                    Some(ev) = self.compositor.receive_event() => {
                        trace_timing!("handle_event", 5 /* ms */, {
                            self.compositor.handle_event(&mut self.editor, ev);
                        });
                    }

                    // No pending events.
                    _ = futures::future::ready(()), if i > 0 => {
                        // Since we've already handled at least one event, if there're no
                        // pending events, we should break the loop to update the
                        // terminal contents.
                        break 'inner;
                    }

                    _ = &mut timeout, if i > 0 => {
                        // Taking too long to handle events. Break the loop to update the
                        // terminal contents.
                        break 'inner;
                    }
                }
            }
        }
    }
}

pub fn truncate_to_width_suffix(s: &str, width: usize) -> &str {
    if s.display_width() <= width {
        return s;
    }

    let mut prev_substr = None;
    for (offset, _) in s.char_indices() {
        let substr = &s[s.len() - offset..];
        if substr.display_width() > width {
            return prev_substr.unwrap_or("");
        }
        prev_substr = Some(substr);
    }

    prev_substr.unwrap_or(s)
}

pub const META_LINE_HEIGHT: usize = 2;

pub enum MetaLineMode {
    Normal,
    Search,
}

pub struct MetaLine {
    mode: MetaLineMode,
    clear_notification_after: usize,
}
impl MetaLine {
    pub fn new() -> Self {
        MetaLine {
            mode: MetaLineMode::Normal,
            clear_notification_after: 0,
        }
    }
}

impl Surface for MetaLine {
    type Context = Editor;

    fn name(&self) -> &str {
        "meta_line"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, _editor: &mut Editor) -> bool {
        true
    }

    fn layout(&mut self, _editor: &mut Editor, screen_size: RectSize) -> (Layout, RectSize) {
        (
            Layout::Fixed {
                y: screen_size.height.saturating_sub(META_LINE_HEIGHT),
                x: 0,
            },
            RectSize {
                height: META_LINE_HEIGHT,
                width: screen_size.width,
            },
        )
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        let doc = editor.current_document();
        // Apply the style.
        canvas.apply_style(0, 0, canvas.width(), theme_for("meta_line.background"));

        match self.mode {
            MetaLineMode::Search => {
                // TODO:
            }
            MetaLineMode::Normal => {
                // Cursor position.
                let cursor_pos = doc.main_cursor().moving_position();
                let cursor_col = cursor_pos.x + 1;
                let cursor_text = if doc.cursors().len() > 1 {
                    let num_invisible_cursors = doc
                        .cursors()
                        .iter()
                        .filter(|c| {
                            let pos = c.moving_position();

                            // TODO:
                            // pos < view.first_visible_position()
                            //     || pos > view.last_visible_position()
                            false
                        })
                        .count();
                    if num_invisible_cursors > 0 {
                        format!(
                            "{} ({}+{})",
                            cursor_col,
                            doc.cursors().len(),
                            num_invisible_cursors
                        )
                    } else {
                        format!("{} ({})", cursor_col, doc.cursors().len())
                    }
                } else {
                    format!("{}", cursor_col)
                };

                // Is the buffer dirty?
                let is_dirty = if doc.is_dirty() { "[+]" } else { "" };

                let left_text = [is_dirty].join(" ");
                let right_text = [cursor_text.as_str()].join(" ");

                // File name.
                let filename = truncate_to_width_suffix(
                    &doc.name,
                    canvas
                        .width()
                        .saturating_sub(left_text.display_width() + right_text.display_width() + 3),
                );
                let filename_width = filename.display_width();

                canvas.write_str(
                    0,
                    canvas
                        .width()
                        .saturating_sub(1 + right_text.display_width()),
                    &right_text,
                );
                canvas.write_str(0, 1, filename);
                canvas.write_str(0, 1 + filename_width + 1, &left_text);
            }
        };

        // Notification.
        if let Some(noti) = notification_manager().last_notification().as_ref() {
            let (theme_key, text) = match noti {
                Notification::Info(message) => ("notification.info", message.as_str()),
                Notification::Warn(message) => ("notification.warn", message.as_str()),
                Notification::Error(err) => ("notification.error", err.as_str()),
            };

            let message = text.lines().next().unwrap_or("");
            canvas.write_str(1, 1, message);
            canvas.apply_style(1, 1, canvas.width(), theme_for(theme_key));
        };
    }
}

struct Text {
    mainloop_tx: UnboundedSender<MainloopCommand>,
    virtual_buffer_width: usize,
    buffer_width: usize,
    buffer_height: usize,
    first_visible_pos: Position,
    last_visible_pos: Position,
    cursor_screen_pos: Option<(usize, usize)>,
    softwrap: bool,
}

impl Text {
    pub fn new(mainloop_tx: UnboundedSender<MainloopCommand>) -> Self {
        Text {
            mainloop_tx,
            virtual_buffer_width: 0,
            buffer_width: 0,
            buffer_height: 0,
            first_visible_pos: Position::new(0, 0),
            last_visible_pos: Position::new(0, 0),
            cursor_screen_pos: None,
            softwrap: true,
        }
    }
}

impl Surface for Text {
    type Context = Editor;

    fn name(&self) -> &str {
        "buffer"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self, ctx: &mut Editor) -> bool {
        true
    }

    fn layout(
        &mut self,
        ctx: &mut Editor,
        screen_size: noa_compositor::surface::RectSize,
    ) -> (
        noa_compositor::surface::Layout,
        noa_compositor::surface::RectSize,
    ) {
        (
            Layout::Fixed { y: 0, x: 0 },
            RectSize {
                width: screen_size.width,
                height: screen_size.height.saturating_sub(2),
            },
        )
    }

    fn cursor_position(&self, editor: &mut Editor) -> Option<(usize, usize)> {
        self.cursor_screen_pos
    }

    fn handle_key_event(
        &mut self,
        editor: &mut Editor,
        compositor: &mut Compositor<Editor>,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let doc = editor.current_document_mut();

        let mut show_completion = false;
        let mut adjust_scroll = true;
        // TODO: Move into defaults.toml
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), CTRL) => {
                self.mainloop_tx.send(MainloopCommand::Quit);
            }
            (KeyCode::Esc, NONE) => {
                doc.clear_secondary_cursors();
            }
            (KeyCode::Up, NONE) => {
                doc.move_cursors_up(self.virtual_buffer_width);
            }
            (KeyCode::Down, NONE) => {
                doc.move_cursors_down(self.virtual_buffer_width);
            }
            (KeyCode::Left, NONE) => {
                doc.move_cursors_left();
            }
            (KeyCode::Right, NONE) => {
                doc.move_cursors_right();
            }
            (KeyCode::Up, SHIFT) => {
                doc.select_cursors_up(self.virtual_buffer_width);
            }
            (KeyCode::Down, SHIFT) => {
                doc.select_cursors_down(self.virtual_buffer_width);
            }
            (KeyCode::Left, SHIFT) => {
                doc.select_cursors_left();
            }
            (KeyCode::Right, SHIFT) => {
                doc.select_cursors_right();
            }
            (KeyCode::Char(ch), NONE) => {
                doc.smart_insert_char(ch);
            }
            (KeyCode::Enter, NONE) => {
                doc.smart_insert_char('\n');
            }
            (KeyCode::Char(ch), SHIFT) => {
                doc.smart_insert_char(ch.to_ascii_uppercase());
            }
            (KeyCode::Backspace, NONE) => {
                doc.backspace();
            }
            (KeyCode::Delete, NONE) => {
                doc.delete();
            }
            (KeyCode::Char('f'), CTRL) => {
                use std::process::Command;
                let mut cmd = Command::new("sk");
                cmd.arg("-c").arg("echo hello");
                self.mainloop_tx.send(MainloopCommand::ExternalCommand(cmd));
            }
            (KeyCode::Char('r'), CTRL) => {
                self.softwrap = !self.softwrap;
                if self.softwrap {
                    self.virtual_buffer_width = self.buffer_width;
                } else {
                    self.virtual_buffer_width = usize::MAX;
                }
            }
            (KeyCode::PageUp, NONE) => {
                doc.scroll_up(self.buffer_height, self.virtual_buffer_width);
                adjust_scroll = false;
            }
            (KeyCode::PageDown, NONE) => {
                doc.scroll_down(self.buffer_height, self.virtual_buffer_width);
                adjust_scroll = false;
            }
            _ => {
                drop(doc);
                if let Some(binding) =
                    get_keybinding_for(KeyBindingScope::Buffer, key.code, key.modifiers)
                {
                    execute_action_or_notify(editor, compositor, &binding.action);
                }
            }
        }

        let doc = editor.current_document_mut();
        if adjust_scroll {
            doc.adjust_scroll(
                self.virtual_buffer_width,
                self.buffer_width,
                self.buffer_height,
                self.first_visible_pos,
                self.last_visible_pos,
            );
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        editor: &mut Editor,
        _compositor: &mut Compositor<Self::Context>,
        input: &str,
    ) -> HandledEvent {
        let doc = editor.current_document_mut();
        doc.insert(input);
        doc.adjust_scroll(
            self.virtual_buffer_width,
            self.buffer_width,
            self.buffer_height,
            self.first_visible_pos,
            self.last_visible_pos,
        );
        HandledEvent::Consumed
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();
        self.cursor_screen_pos = None;

        let doc = editor.current_document();

        self.buffer_height = canvas.height();
        let lineno_width =
            2 + (doc.scroll.paragraph_index.buffer_y + 1 + self.buffer_height).display_width();
        let buffer_start_x = lineno_width;
        self.buffer_width = canvas.width().saturating_sub(lineno_width);

        if self.buffer_width == 0 {
            warn!("too small screen");
            return;
        }

        if self.virtual_buffer_width != usize::MAX {
            self.virtual_buffer_width = self.buffer_width;
        }

        self.first_visible_pos = Position::new(usize::MAX, usize::MAX);
        self.last_visible_pos = Position::new(0, 0);

        // Buffer contents.
        let main_cursor_pos = doc.main_cursor().moving_position();
        let mut screen_y_offset = 0;
        let mut linenos: Vec<usize> = Vec::new();
        trace_timing!("render_text", 3 /* ms */, {
            for (Paragraph {
                mut reflow_iter,
                index: paragraph_index,
            }) in doc.paragraph_iter_at_index(
                doc.scroll.paragraph_index,
                self.virtual_buffer_width,
                doc.editorconfig().tab_width,
            ) {
                reflow_iter.enable_eof(true);

                let mut paragraph_height = 0;
                let skipped_y = if doc.scroll.paragraph_index == paragraph_index {
                    doc.scroll.y_in_paragraph
                } else {
                    0
                };

                for ReflowItem {
                    grapheme,
                    grapheme_width,
                    pos_in_screen,
                    pos_in_buffer,
                } in reflow_iter
                {
                    if pos_in_screen.y < skipped_y {
                        continue;
                    }

                    if pos_in_screen.x < doc.scroll.x_in_paragraph {
                        continue;
                    }

                    let canvas_y = screen_y_offset + pos_in_screen.y - skipped_y;
                    let canvas_x = buffer_start_x + pos_in_screen.x - doc.scroll.x_in_paragraph;

                    if canvas_y >= canvas.height() {
                        break;
                    }

                    if canvas_x >= canvas.width() {
                        break;
                    }

                    self.first_visible_pos = min(self.first_visible_pos, pos_in_buffer);
                    self.last_visible_pos = max(self.last_visible_pos, pos_in_buffer);

                    if canvas_y >= linenos.len() {
                        linenos.push(pos_in_buffer.y + 1);
                    }

                    match grapheme {
                        PrintableGrapheme::Grapheme(grapheme) => {
                            paragraph_height = pos_in_screen.y;
                            canvas.write(
                                canvas_y,
                                canvas_x,
                                Grapheme::new_with_width(grapheme, grapheme_width),
                            );
                        }
                        PrintableGrapheme::Eof
                        | PrintableGrapheme::Whitespaces
                        | PrintableGrapheme::ZeroWidth
                        | PrintableGrapheme::Newline(_) => {
                            // Already filled with whitespaces by `canvas.clear()`.
                        }
                    }

                    for c in doc.cursors() {
                        if c.selection().contains(pos_in_buffer)
                            || (!c.is_main_cursor() && c.position() == Some(pos_in_buffer))
                        {
                            canvas.set_inverted(
                                canvas_y,
                                canvas_x,
                                canvas_x + grapheme_width,
                                true,
                            );
                        }
                    }

                    if main_cursor_pos == pos_in_buffer {
                        self.cursor_screen_pos = Some((canvas_y, canvas_x));
                    }
                }

                screen_y_offset += 1 + paragraph_height - skipped_y;
            }
        });

        // Line numbers.
        let mut prev = 0;
        for (canvas_y, lineno) in linenos.into_iter().enumerate() {
            if lineno != prev {
                canvas.write_str(
                    canvas_y,
                    1,
                    &format!("{:>width$}", lineno, width = lineno_width - 2),
                );
            }

            prev = lineno;
        }
    }
}
