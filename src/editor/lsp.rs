use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;

use noa_buffer::{
    buffer::TextEdit,
    cursor::{Position, Range},
};
use noa_common::oops::OopsExt;
use noa_languages::language::{Language, Lsp};
use noa_proxy::{
    client::Client,
    lsp_types::{self, CompletionTextEdit},
};

use crate::{
    completion::{CompletionItem, CompletionKind},
    document::{Document, OnChangeData},
};

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

pub async fn completion_hook(
    lang: &'static Language,
    client: Arc<Client>,
    path: PathBuf,
    pos: Position,
    current_word_range: Range,
) -> Result<Vec<CompletionItem>> {
    let lsp = match lang.lsp.as_ref() {
        Some(lsp) => lsp,
        None => return Ok(vec![]),
    };

    let lsp_items = client.completion(lsp, &path, pos.into()).await?;
    let mut items = Vec::new();
    for lsp_item in lsp_items.into_iter() {
        let mut text_edits: Vec<TextEdit> = lsp_item
            .additional_text_edits
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(Into::into)
            .collect();

        let item = match (&lsp_item.insert_text, &lsp_item.text_edit) {
            (Some(insert_text), None) => {
                text_edits.push(TextEdit {
                    range: current_word_range,
                    new_text: insert_text.to_owned(),
                });

                CompletionItem {
                    kind: CompletionKind::LspItem,
                    label: lsp_item.label,
                    text_edits,
                }
            }
            (None, Some(CompletionTextEdit::Edit(edit))) => {
                text_edits.push(TextEdit {
                    range: edit.range.into(),
                    new_text: edit.new_text.to_owned(),
                });

                CompletionItem {
                    kind: CompletionKind::LspItem,
                    label: lsp_item.label,
                    text_edits,
                }
            }
            (None, Some(CompletionTextEdit::InsertAndReplace(edit))) => {
                text_edits.push(TextEdit {
                    range: edit.insert.into(),
                    new_text: edit.new_text.to_owned(),
                });

                CompletionItem {
                    kind: CompletionKind::LspItem,
                    label: lsp_item.label,
                    text_edits,
                }
            }
            _ => {
                warn!("unsupported LSP completion item: {:?}", lsp_item);
                continue;
            }
        };

        items.push(item);
    }

    Ok(items)
}
