use crossterm::event::KeyEvent;

use crate::{
    editor::UserMessage,
    ui::{
        truncate_to_width, CanvasViewMut, Compositor, Context, DisplayWidth, HandledEvent, Layout,
        RectSize, Surface,
    },
};

pub struct BottomBarSurface {}

impl BottomBarSurface {
    pub fn new() -> BottomBarSurface {
        BottomBarSurface {}
    }
}

impl Surface for BottomBarSurface {
    fn name(&self) -> &str {
        "bottom_bar"
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        (
            Layout::Fixed {
                y: screen_size.height.saturating_sub(2),
                x: 0,
            },
            RectSize {
                height: 2,
                width: screen_size.width,
            },
        )
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        None
    }

    fn render<'a>(&mut self, ctx: &mut Context, mut canvas: CanvasViewMut<'a>) {
        let buffer = ctx.editor.current_buffer().read();

        canvas.clear();
        canvas.set_style(0, 0, canvas.width(), &ctx.theme.bottom_bar_text);

        let marker = if buffer.is_dirty() { "[+]" } else { "" };
        let marker_width = marker.display_width();
        let colno = buffer.main_cursor_pos().x;
        let colno_width = colno.display_width();
        let num_cursors = buffer.cursors().len();
        let num_cursors_width = if num_cursors == 1 {
            0
        } else {
            3 + num_cursors.display_width()
        };
        let name_max_len = canvas
            .width()
            .saturating_sub(marker_width + 1 + 1 + colno_width + num_cursors_width);

        info!(
            "truncate_to_width(buffer.name(), name_max_len)= '{}' '{}' {}",
            truncate_to_width(buffer.name(), name_max_len),
            buffer.name(),
            name_max_len,
        );
        canvas.draw_str(0, 0, marker);
        canvas.draw_str(
            0,
            marker_width + 1,
            truncate_to_width(buffer.name(), name_max_len),
        );
        canvas.draw_str(
            0,
            canvas.width() - num_cursors_width - colno_width,
            &format!("{}", colno),
        );

        if num_cursors_width > 0 {
            canvas.draw_str(
                0,
                canvas.width() - num_cursors_width,
                &format!(" ({})", num_cursors),
            );
        }

        if let Some(message) = ctx.editor.last_message() {
            match message {
                UserMessage::Error(text) => {
                    let x = canvas.width() - text.display_width();
                    canvas.draw_str(1, x, &text);
                }
            }
        }
    }

    fn handle_key_event(
        &mut self,
        _ctx: &mut Context,
        _compositor: &mut Compositor,
        _key: KeyEvent,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }

    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut Context,
        _compositor: &mut Compositor,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }
}
