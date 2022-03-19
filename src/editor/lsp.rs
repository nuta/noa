use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;

use futures::executor::block_on;
use noa_buffer::{
    buffer::TextEdit,
    cursor::{Position, Range},
    mutable_raw_buffer::Change,
};
use noa_common::oops::OopsExt;
use noa_languages::language::Language;
use noa_proxy::{
    client::Client,
    lsp_types::{self, CompletionTextEdit, HoverContents},
};
use tokio::time::timeout;

use crate::{
    completion::{CompletionItem, CompletionKind},
    document::Document,
    job::JobManager,
    ui::{bump_view::BumpView, markdown::Markdown},
};

pub fn modified_hook(client: &Arc<Client>, doc: &Document, changes: Vec<Change>) {
    let lsp = match doc.buffer().language().lsp.as_ref() {
        Some(lsp) => lsp,
        None => return,
    };

    // Synchronize the latest buffer text with the LSP server.
    let client = client.clone();
    let initial_buffer = doc.raw_buffer().clone();
    let version = doc.version();
    let path = doc.path().to_owned();
    let changes = changes.to_vec();
    tokio::spawn(async move {
        client
            .open_file(lsp, &path, &initial_buffer.text())
            .await
            .oops();

        let path = path.clone();
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
    });
}

pub fn before_save_hook(client: &Arc<Client>, doc: &mut Document) {
    let lsp = match doc.buffer().language().lsp.as_ref() {
        Some(lsp) => lsp,
        None => return,
    };

    trace!("format on save: {}", doc.path().display());
    let format_future = client.format(lsp, doc.path(), (*doc.buffer().editorconfig()).into());
    match block_on(timeout(Duration::from_secs(3), format_future)) {
        Ok(Ok(edits)) => {
            doc.buffer_mut()
                .apply_text_edits(edits.into_iter().map(Into::into).collect());
        }
        Ok(Err(err)) => {
            notify_warn!("LSP formatting failed");
            warn!("LSP formatting failed: {}", err);
        }
        Err(_) => {
            notify_warn!("LSP formatting timed out");
        }
    }
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

pub fn hover_hook(
    client: &Arc<Client>,
    jobs: &mut JobManager,
    lang: &'static Language,
    path: &Path,
    pos: Position,
) {
    let lsp = match lang.lsp.as_ref() {
        Some(lsp) => lsp,
        None => return,
    };

    let client = client.clone();
    let path = path.to_owned();
    jobs.await_in_mainloop(
        async move {
            let result = match client.hover(lsp, &path, pos.into()).await {
                Ok(Some(hover)) => match hover {
                    HoverContents::Scalar(text) => {
                        let markdown = Markdown::from(text);
                        notify_info!("{}", markdown);
                        Some(markdown)
                    }
                    HoverContents::Array(items) if !items.is_empty() => {
                        let markdown = Markdown::from(items[0].clone());
                        notify_info!("{}", markdown);
                        Some(markdown)
                    }
                    HoverContents::Markup(markup) => {
                        let markdown = Markdown::from(markup);
                        notify_info!("{}", markdown);
                        Some(markdown)
                    }
                    _ => {
                        warn!("unsupported hover type: {:?}", hover);
                        None
                    }
                },
                Ok(None) => {
                    notify_warn!("no hover info");
                    None
                }
                Err(err) => {
                    notify_error!("failed to get hover info: {}", err);
                    None
                }
            };

            Ok(result)
        },
        |_, compositor, markdown| {
            if let Some(markdown) = markdown {
                let bump_view: &mut BumpView = compositor.get_mut_surface_by_name("bump");
                bump_view.open(markdown);
            }
        },
    );
}
