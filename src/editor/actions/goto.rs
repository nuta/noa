use std::path::Path;

use anyhow::Result;
use noa_buffer::cursor::Position;
use noa_compositor::Compositor;

use crate::{editor::Editor, ui::prompt_view::PromptView};

use super::Action;

pub struct GoToLine;

impl Action for GoToLine {
    fn name(&self) -> &'static str {
        "goto_line"
    }

    fn run(&self, _editor: &mut Editor, compositor: &mut Compositor<Editor>) -> Result<()> {
        let prompt = compositor.get_mut_surface_by_name::<PromptView>("prompt");
        prompt.open(
            "Go To Line",
            Box::new(|editor, _, prompt, entered| {
                if entered {
                    let input = prompt.text();
                    if let Ok(lineno) = input.parse::<usize>() {
                        let pos = Position::new(lineno.saturating_sub(1), 0);
                        editor.current_buffer_mut().move_main_cursor_to_pos(pos);
                    } else {
                        let mut words = input.split(':');
                        let maybe_path = words.next();
                        let maybe_lineno = words.next().map(|s| s.parse::<usize>());
                        let (path, pos) = match (maybe_path, maybe_lineno) {
                            (Some(_), Some(Err(_))) => {
                                notify_error!("invalid lineno");
                                return;
                            }
                            (None, _) => {
                                notify_error!("invalid path");
                                return;
                            }
                            (_, Some(Ok(lineno))) if lineno == 0 => {
                                notify_error!("lineno must be greater than 0");
                                return;
                            }
                            (Some(path), _) if !Path::new(path).exists() => {
                                notify_error!("nonexistent path");
                                return;
                            }
                            (Some(path), Some(Ok(lineno))) => {
                                (Path::new(path), Some(Position::new(lineno - 1, 0)))
                            }
                            (Some(path), None) => (Path::new(path), None),
                        };

                        match editor.open_file(path, pos) {
                            Ok(id) => {
                                editor.documents.switch_by_id(id);
                            }
                            Err(err) => {
                                notify_error!("failed to open: {}", err);
                            }
                        }
                    }

                    prompt.close();
                }
            }),
        );
        Ok(())
    }
}
