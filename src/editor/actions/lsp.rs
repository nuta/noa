use anyhow::Result;

use noa_compositor::Compositor;

use crate::{editor::Editor, lsp};

use super::Action;

pub struct CodeActions;

impl Action for CodeActions {
    fn name(&self) -> &'static str {
        "lsp.code_actions"
    }

    fn run(&self, editor: &mut Editor, _compositor: &mut Compositor<Editor>) -> Result<()> {
        let doc = editor.documents.current_mut();

        editor.jobs.await_in_mainloop(
            lsp::list_code_actions(
                doc.buffer().language(),
                editor.proxy.clone(),
                doc.path().to_owned(),
                doc.buffer().main_cursor().selection().into(),
            ),
            |editor, compositor, actions| {
                //
                trace!("list_code_actions: {:?}", actions);
            },
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
            |editor, compositor, actions| {
                //
                trace!("rename_symbol: {:?}", actions);
            },
        );

        Ok(())
    }
}
