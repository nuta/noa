use std::sync::Arc;

use noa_common::oops::OopsExt;

use noa_proxy::lsp_types::TextEdit;

use crate::{document::OnChangeData, editor::Editor, hook::Hook, linemap::LineMap};

pub fn init_hooks(editor: &mut Editor) {
    editor
        .hooks
        .register(Hook::AfterOpen, "lsp_file_sync", |editor, _| {
            let doc = editor.documents.current_mut();

            let lsp = match doc.buffer().language().lsp.as_ref() {
                Some(lsp) => lsp,
                None => return Ok(()),
            };

            let mut rx = doc.subscribe_onchange();
            let proxy = editor.proxy.clone();
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
                        .map(|change| TextEdit {
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

            Ok(())
        });

    editor
        .hooks
        .register(Hook::AfterOpen, "git_diff", |editor, _| {
            let doc = editor.documents.current_mut();

            let mut rx = doc.subscribe_onchange();
            let repo = editor.repo.clone();
            let linemap = doc.linemap().clone();
            let path = doc.path().to_owned();
            let render_request = editor.render_request.clone();

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

            Ok(())
        });

    editor
        .hooks
        .register(Hook::AfterOpen, "git_diff", |editor, _| {
            let doc = editor.documents.current_mut();
            // Watch changes on disk and reload it if changed.
            if let Some(listener) = doc.modified_listener().cloned() {
                let doc_id = doc.id();
                editor
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
            Ok(())
        });
}
