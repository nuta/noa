use std::sync::Arc;

use noa_common::oops::OopsExt;

use noa_proxy::lsp_types::TextEdit;

use crate::{
    document::OnChangeData,
    editor::Editor,
    hook::{Hook, HookManager},
    linemap::LineMap,
};

mod lsp;

pub fn init_hooks(hooks: &mut HookManager) {
    // Git diff.
    hooks.after_save.register(|ctx, _| {
        let doc = ctx.editor.documents.current_mut();

        let mut rx = doc.subscribe_onchange();
        let repo = ctx.editor.repo.clone();
        let linemap = doc.linemap().clone();
        let path = doc.path().to_owned();
        let render_request = ctx.editor.render_request.clone();

        tokio::spawn(async move {
            while let Ok(OnChangeData { raw_buffer, .. }) = rx.recv().await {
                if let Some(repo) = &repo {
                    let buffer_text = raw_buffer.text();
                    let mut new_linemap = LineMap::new();
                    new_linemap.update_git_line_statuses(repo, &path, &buffer_text);
                    linemap.store(Arc::new(new_linemap));
                    render_request.notify_one();
                }
            }
        });
    });

    // Watch changes on disk and reload it if changed.
    hooks.after_save.register(|ctx, _| {
        let doc = ctx.editor.documents.current_mut();
        if let Some(listener) = doc.modified_listener().cloned() {
            let doc_id = doc.id();
            ctx.editor
                .jobs
                .listen_in_mainloop(listener, move |editor, _compositor| {
                    let current_id = editor.documents.current().id();
                    let doc = match editor.documents.get_mut_document_by_id(doc_id) {
                        Some(doc) => doc,
                        None => {
                            warn!("document {:?} was closed", doc_id);
                            return;
                        }
                    };

                    match doc.reload() {
                        Ok(_) => {
                            if current_id == doc.id() {
                                notify_info!("reloaded from the disk");
                            }
                        }
                        Err(err) => {
                            warn!("failed to reload {}: {:?}", doc.path().display(), err);
                        }
                    }
                });
        }
    });
}
