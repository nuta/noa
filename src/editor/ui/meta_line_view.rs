use std::cmp::min;

use noa_buffer::display_width::DisplayWidth;
use noa_compositor::{
    canvas::CanvasViewMut,
    line_edit::LineEdit,
    surface::{HandledEvent, KeyEvent, Layout, RectSize, Surface},
    terminal::{KeyCode, KeyModifiers},
    Compositor,
};

use crate::{
    editor::Editor,
    notification::{notification_manager, Notification},
    theme::theme_for,
};

use super::helpers::{truncate_to_width, truncate_to_width_reverse};

struct LastNotification {
    theme_key: &'static str,
    wrapped_text: String,
}

pub struct MetaLineView {
    search_query: LineEdit,
    last_notification: Option<LastNotification>,
    clear_notification_after: usize,
}

impl MetaLineView {
    pub fn new() -> MetaLineView {
        MetaLineView {
            search_query: LineEdit::new(),
            last_notification: None,
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
        let height = 1;
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

        let doc = editor.documents.current();
        let view = doc.view();
        let buffer = doc.buffer();
        let cursor_pos = buffer.main_cursor().moving_position();
        let cursor_pos_str = if buffer.cursors().len() > 1 {
            let num_invisible_cursors = buffer
                .cursors()
                .iter()
                .filter(|c| {
                    let pos = c.moving_position();

                    // Handle the case when the cursor is at EOF.
                    pos < view.first_visible_position() || pos > view.last_visible_position()
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
        let cursor_pos_width = cursor_pos_str.display_width();
        let filename_max_width = 32usize;
        let search_query = self.search_query.text();

        // Notification.
        // if let Some(LastNotification {
        //     theme_key,
        //     wrapped_text,
        // }) = self.last_notification.as_ref()
        // {
        //     let style = theme_for(theme_key);
        //     for (y, line) in wrapped_text
        //         .lines()
        //         .take(self.notification_height)
        //         .enumerate()
        //     {
        //         canvas.write_str(y, 0, line);
        //         canvas.apply_style(y, 0, canvas.width(), style);
        //     }
        // }

        // if let Some(noti) = notification_manager().last_notification().as_ref() {
        //     let (theme_key, text) = match noti {
        //         Notification::Info(message) => ("notification.info", message.as_str()),
        //         Notification::Warn(message) => ("notification.warn", message.as_str()),
        //         Notification::Error(err) => ("notification.error", err.as_str()),
        //     };

        //     if self.clear_notification_after == 0 {
        //         self.clear_notification_after = 3;
        //     }

        //     let wrapped_text = textwrap::fill(text, width);
        //     self.notification_height = min(8, wrapped_text.lines().count());
        //     self.last_notification = Some(LastNotification {
        //         theme_key,
        //         wrapped_text,
        //     });
        //     height += self.notification_height;
        // } else {
        //     self.notification_height = 0;
        //     self.last_notification = None;
        // };

        // // Search query.
        canvas.write_str(0, 1, &search_query);

        // File name.
        let filename = truncate_to_width_reverse(
            doc.name(),
            filename_max_width.saturating_sub(cursor_pos_width + 2),
        );
        canvas.write_str(
            0,
            canvas
                .width()
                .saturating_sub(cursor_pos_width + filename.display_width() + 3),
            filename,
        );

        // Cursor position.
        canvas.write_str(
            0,
            canvas.width().saturating_sub(1 + cursor_pos_width),
            &cursor_pos_str,
        );

        // Apply the style.
        canvas.apply_style(
            0,
            0,
            canvas.width().saturating_sub(1 + cursor_pos_width),
            theme_for("meta_line.background"),
        );
        canvas.apply_style(
            0,
            canvas.width().saturating_sub(2 + cursor_pos_width),
            canvas.width(),
            theme_for("meta_line.cursor_pos"),
        );
    }

    fn handle_key_event(
        &mut self,
        _compositor: &mut Compositor<Self::Context>,
        _editor: &mut Editor,
        key: KeyEvent,
    ) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        // const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        trace!("MetaLineView::handle_key_event: {:?}", key);

        match (key.code, key.modifiers) {
            (KeyCode::Esc, NONE) if !self.search_query.is_empty() => {
                self.search_query.clear();
                HandledEvent::Consumed
            }
            (KeyCode::Esc, NONE) if self.last_notification.is_some() => {
                notification_manager().clear();
                self.last_notification = None;
                HandledEvent::Consumed
            }
            _ => {
                self.clear_notification_after = self.clear_notification_after.saturating_sub(1);
                if self.clear_notification_after == 0 {
                    notification_manager().clear();
                    self.last_notification = None;
                }

                HandledEvent::Ignored
            }
        }
    }

    fn handle_key_batch_event(
        &mut self,
        _compositor: &mut Compositor<Editor>,
        _editor: &mut Editor,
        _s: &str,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }
}
