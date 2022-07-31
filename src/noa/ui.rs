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
    views::buffer_view::BufferView,
};

pub enum MainloopCommand {
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
            .add_frontmost_layer(Box::new(BufferView::new(mainloop_tx.clone())));
        self.compositor
            .add_frontmost_layer(Box::new(MetaLine::new()));
        'outer: loop {
            trace_timing!("render", 5 /* ms */, {
                self.compositor.render(&mut self.editor);
            });

            let timeout = time::sleep(Duration::from_millis(10));
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
