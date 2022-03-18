use crate::ui::compositor::Compositor;
use anyhow::Result;

use crate::{editor::Editor, ui::surface::UIContext};

use super::Action;

pub struct MoveToNextDiff;

impl Action for MoveToNextDiff {
    fn name(&self) -> &'static str {
        "move_to_next_diff"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor) -> Result<()> {
        let doc = editor.documents.current_mut();
        let linemap = doc.linemap().load();
        match linemap.next_diff_line(doc.buffer().main_cursor().moving_position().y) {
            Some(pos) => {
                doc.buffer_mut().move_main_cursor_to_pos(pos);
            }
            None => {
                notify_warn!("no previous diff line");
            }
        }
        Ok(())
    }
}

pub struct MoveToPrevDiff;

impl Action for MoveToPrevDiff {
    fn name(&self) -> &'static str {
        "move_to_prev_diff"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor) -> Result<()> {
        let doc = editor.documents.current_mut();
        let linemap = doc.linemap().load();
        match linemap.prev_diff_line(doc.buffer().main_cursor().moving_position().y) {
            Some(pos) => {
                doc.buffer_mut().move_main_cursor_to_pos(pos);
            }
            None => {
                notify_warn!("no previous diff line");
            }
        }
        Ok(())
    }
}
