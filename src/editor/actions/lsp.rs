use anyhow::Result;

use noa_compositor::Compositor;

use crate::{editor::Editor, lsp, ui::prompt_view::PromptView};

use super::Action;

pub struct GoToDefinition;

impl Action for GoToDefinition {
    fn name(&self) -> &'static str {
        "lsp.goto_definition"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        let doc = editor.documents.current_mut();
        lsp::goto_definition(
            &editor.proxy,
            &mut editor.jobs,
            doc.buffer().language(),
            doc.path(),
            doc.buffer().main_cursor().moving_position(),
        );

        Ok(())
    }
}

pub struct RenameSymbol;

impl Action for RenameSymbol {
    fn name(&self) -> &'static str {
        "lsp.rename_symbol"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        let doc = editor.documents.current_mut();

        editor.jobs.await_in_mainloop(
            lsp::prepare_rename_symbol(
                doc.buffer().language(),
                editor.proxy.clone(),
                doc.path().to_owned(),
                doc.buffer().main_cursor().moving_position(),
            ),
            |editor, compositor, range| {
                let range = match range {
                    Ok(Some(range)) => range,
                    Ok(None) => {
                        notify_warn!("no symbol to rename");
                        return;
                    }
                    Err(err) => {
                        notify_warn!("cannot rename: {}", err);
                        return;
                    }
                };

                let buffer = editor.current_buffer_mut();
                if !buffer.is_valid_range(range) {
                    notify_warn!("invalid rename range");
                    return;
                }

                let old_name = buffer.substr(range);
                trace!("rename_symbol: {:?}", old_name);

                // Ask the user for a new name.
                let prompt = compositor.get_mut_surface_by_name::<PromptView>("prompt");
                prompt.open(
                    format!("Rename {}", old_name),
                    Box::new(move |editor, _, prompt, entered| {
                        let doc = editor.documents.current_mut();
                        if entered {
                            // Do the rename.
                            let new_name = prompt.text();
                            editor.jobs.await_in_mainloop(
                                lsp::rename_symbol(
                                    doc.buffer().language(),
                                    editor.proxy.clone(),
                                    doc.path().to_owned(),
                                    doc.buffer().main_cursor().moving_position(),
                                    new_name.clone(),
                                ),
                                move |editor, _, edit| match edit.and_then(|edit| {
                                    editor.documents.apply_workspace_edit_in_only_current(&edit)
                                }) {
                                    Ok(_) => {
                                        notify_info!("renamed to {}", new_name);
                                    }
                                    Err(err) => {
                                        notify_error!("failed to rename: {}", err);
                                    }
                                },
                            );

                            prompt.close();
                        }
                    }),
                );
            },
        );

        Ok(())
    }
}
