use crate::editor::Editor;

use anyhow::Result;
use noa_compositor::Compositor;

use super::Action;

pub struct Truncate;

impl Action for Truncate {
    fn name(&self) -> &'static str {
        "truncate"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        editor
            .documents
            .current_mut()
            .buffer_mut()
            .foreach_cursors(|buf, c, past_cursors| {
                if c.selection().is_empty() {
                    // Select until the end of line.
                    let pos = c.moving_position();
                    let eol = buf.line_len(pos.y);
                    if pos.x == eol {
                        // The cursor is already at the end of line, remove the
                        // following newline instead.
                        c.select(pos.y, pos.x, pos.y + 1, 0);
                    } else {
                        c.select(pos.y, pos.x, pos.y, eol);
                    }
                }

                buf.edit_at_cursor(c, past_cursors, "");
            });

        Ok(())
    }
}
