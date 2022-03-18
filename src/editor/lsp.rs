use std::sync::Arc;

use noa_common::oops::OopsExt;
use noa_proxy::{client::Client, lsp_types};

use crate::document::{Document, OnChangeData};

pub fn after_open_hook(client: &Arc<Client>, doc: &Document) {
    let lsp = match doc.buffer().language().lsp.as_ref() {
        Some(lsp) => lsp,
        None => return,
    };

    // Synchronize the latest buffer text with the LSP server.
    let mut rx = doc.subscribe_onchange();
    let client = client.clone();
    let initial_buffer = doc.raw_buffer().clone();
    let path = doc.path().to_owned();
    tokio::spawn(async move {
        client
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

            client
                .incremental_update_file(lsp, &path, edits, version)
                .await
                .oops();
        }
    });
}
