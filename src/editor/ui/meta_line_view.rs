use noa_buffer::display_width::DisplayWidth;
use noa_compositor::{
    canvas::CanvasViewMut,
    surface::{HandledEvent, KeyEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers},
    Compositor,
};

use crate::{
    config::theme_for,
    editor::Editor,
    notification::{notification_manager, Notification},
};

use super::helpers::truncate_to_width_suffix;

pub enum MetaLineMode {
    Normal,
    Search,
}

pub struct MetaLineView {
    mode: MetaLineMode,
    clear_notification_after: usize,
}

impl MetaLineView {
    pub fn new() -> MetaLineView {
        MetaLineView {
            mode: MetaLineMode::Normal,
            clear_notification_after: 0,
        }
    }
}

impl Surface for MetaLineView {
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
        let height = 2;
        (
            Layout::Fixed {
                y: screen_size.height.saturating_sub(height),
                x: 0,
            },
            RectSize {
                height,
                width: screen_size.width,
            },
        )
    }

    fn cursor_position(&self, _editor: &mut Editor) -> Option<(usize, usize)> {
        None
    }

    fn render(&mut self, editor: &mut Editor, canvas: &mut CanvasViewMut<'_>) {
        canvas.clear();

        let search_query = editor.find_query.text();
        let doc = editor.documents.current();
        let view = doc.view();
        let buffer = doc.buffer();

        // Apply the style.
        canvas.apply_style(0, 0, canvas.width(), theme_for("meta_line.background"));

        let leftside_width = match self.mode {
            MetaLineMode::Search => {
                // Search query.
                let truncated_query = truncate_to_width_suffix(&search_query, canvas.width());
                canvas.write_str(0, 1, truncated_query);
                truncated_query.display_width()
            }
            MetaLineMode::Normal => {
                // Cursor position.
                let cursor_pos = buffer.main_cursor().moving_position();
                let cursor_text = if buffer.cursors().len() > 1 {
                    let num_invisible_cursors = buffer
                        .cursors()
                        .iter()
                        .filter(|c| {
                            let pos = c.moving_position();

                            // Handle the case when the cursor is at EOF.
                            pos < view.first_visible_position()
                                || pos > view.last_visible_position()
                        })
                        .count();
                    if num_invisible_cursors > 0 {
                        format!(
                            "{} [{}+{}]",
                            cursor_pos.x,
                            buffer.cursors().len(),
                            num_invisible_cursors
                        )
                    } else {
                        format!("{} [{}]", cursor_pos.x, buffer.cursors().len())
                    }
                } else {
                    format!("{}", cursor_pos.x)
                };

                // Is the buffer dirty?
                let is_dirty = if doc.is_dirty() { " [dirty]" } else { "" };

                // Are there any in-progress async jobs?
                let is_busy = if editor.jobs.is_busy() { " [busy]" } else { "" };

                let left_text = format!("{}{}{}", cursor_text, is_dirty, is_busy);

                // File name.
                let filename = truncate_to_width_suffix(
                    doc.name(),
                    canvas.width().saturating_sub(left_text.display_width() + 2),
                );
                let filename_width = filename.display_width();
                canvas.write_str(0, 1, filename);

                // Cursor position.
                canvas.write_str(0, 1 + filename_width + 1, &left_text);

                1 + filename_width + 1
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

    fn handle_key_event(
        &mut self,
        editor: &mut Editor,
        _compositor: &mut Compositor<Self::Context>,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        match self.mode {
            MetaLineMode::Search => match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    self.mode = MetaLineMode::Normal;
                    editor.find_query.clear();
                }
                (KeyCode::Enter, NONE) => {
                    self.mode = MetaLineMode::Normal;
                    editor.find_query.save_undo();
                }
                _ => {
                    editor.find_query.consume_key_event(key);
                }
            },
            MetaLineMode::Normal => match (key.code, key.modifiers) {
                (KeyCode::Esc, NONE) if !editor.find_query.is_empty() => {
                    editor.find_query.clear();
                }
                (KeyCode::Esc, NONE) if !notification_manager().is_empty() => {
                    notification_manager().clear();
                }
                (KeyCode::Char('l'), CTRL) => {
                    self.mode = MetaLineMode::Search;
                }
                _ => {
                    self.clear_notification_after = self.clear_notification_after.saturating_sub(1);
                    if self.clear_notification_after == 0 {
                        notification_manager().clear();
                    }

                    return HandledEvent::Ignored;
                }
            },
        }

        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        editor: &mut Editor,
        _compositor: &mut Compositor<Editor>,
        s: &str,
    ) -> HandledEvent {
        match self.mode {
            MetaLineMode::Search => {
                editor.find_query.insert(s);
                HandledEvent::Consumed
            }
            MetaLineMode::Normal => HandledEvent::Ignored,
        }
    }
}
