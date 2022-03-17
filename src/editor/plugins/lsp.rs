use noa_common::oops::OopsExt;
use noa_proxy::lsp_types;

use crate::{
    document::OnChangeData,
    plugin::{Plugin, PluginContext},
};

pub struct LspPlugin;

impl Plugin for LspPlugin {
    fn name(&self) -> &'static str {
        "lsp"
    }

    fn init(&mut self, PluginContext { hooks, .. }: &mut PluginContext) {
        // Synchronize changes in the buffer.
        hooks.after_save.register(|ctx, _| {
            let doc = ctx.editor.documents.current_mut();

            let lsp = match doc.buffer().language().lsp.as_ref() {
                Some(lsp) => lsp,
                None => return,
            };

            let mut rx = doc.subscribe_onchange();
            let proxy = ctx.editor.proxy.clone();
            let initial_buffer = doc.raw_buffer().clone();
            let path = doc.path().to_owned();
            tokio::spawn(async move {
                proxy
                    .open_file(lsp, &path, &initial_buffer.text())
                    .await
                    .oops();

                let path = path.clone();
                while let Ok(OnChangeData {
                    version, changes, ..
                }) = rx.recv().await
                {
                    let edits = changes
                        .into_iter()
                        .map(|change| lsp_types::TextEdit {
                            range: change.range.into(),
                            new_text: change.insert_text,
                        })
                        .collect();

                    proxy
                        .incremental_update_file(lsp, &path, edits, version)
                        .await
                        .oops();
                }
            });
        });
    }
}
