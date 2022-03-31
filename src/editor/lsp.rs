use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{bail, Result};

use noa_buffer::{
    buffer::TextEdit,
    cursor::{Position, Range},
    mut_raw_buffer::Change,
};
use noa_common::oops::OopsExt;
use noa_languages::Language;
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
            .incremental_update_file(lsp, &path, edits, version.value())
            .await
            .oops();
    });
}

pub async fn format_on_save(
    lang: &'static Language,
    client: Arc<Client>,
    path: PathBuf,
    options: lsp_types::FormattingOptions,
) -> Result<Vec<TextEdit>> {
    let lsp = match lang.lsp.as_ref() {
        Some(lsp) => lsp,
        None => return Ok(vec![]),
    };

    trace!("format on save: {}", path.display());
    let format_future = client.format(lsp, &path, options);
    match timeout(Duration::from_secs(3), format_future).await {
        Ok(Ok(edits)) => Ok(edits.into_iter().map(Into::into).collect()),
        Ok(Err(err)) => {
            bail!("LSP formatting failed: {}", err);
        }
        Err(_) => {
            bail!("LSP formatting timed out");
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

            result
        },
        |_, compositor, markdown| {
            if let Some(markdown) = markdown {
                let bump_view: &mut BumpView = compositor.get_mut_surface_by_name("bump");
                bump_view.open(markdown);
            }
        },
    );
}

pub fn goto_definition(
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
            let result = match client.goto_definition(lsp, &path, pos.into()).await {
                Ok(locations) => locations,
                Err(err) => {
                    notify_error!("failed to go to definition: {}", err);
                    vec![]
                }
            };

            result
        },
        |editor, _, locations| {
            let loc = match locations.get(0) {
                Some(loc) => loc,
                None => {
                    notify_warn!("no definition found");
                    return;
                }
            };

            match editor.open_file(&loc.path, Some(loc.position.into())) {
                Ok(_) => {
                    editor.documents.switch_by_path(&loc.path);
                    notify_info!("here's the defnition");
                }
                Err(err) => {
                    notify_error!("failed to open file: {}", err);
                }
            }
        },
    );
}

pub async fn prepare_rename_symbol(
    lang: &'static Language,
    client: Arc<Client>,
    path: PathBuf,
    pos: Position,
) -> Result<Option<Range>> {
    let lsp = match lang.lsp.as_ref() {
        Some(lsp) => lsp,
        None => return Ok(None),
    };

    trace!("prepare_rename_symbol: {}", path.display());
    let fut = client.prepare_rename_symbol(lsp, &path, pos.into());
    match timeout(Duration::from_secs(5), fut).await {
        Ok(Ok(range)) => Ok(Some(range.into())),
        Ok(Err(err)) => {
            bail!("LSP prepare_rename_symbol failed: {}", err);
        }
        Err(_) => {
            bail!("LSP prepare_rename_symbol timed out");
        }
    }
}

pub async fn rename_symbol(
    lang: &'static Language,
    client: Arc<Client>,
    path: PathBuf,
    pos: Position,
    new_name: String,
) -> Result<lsp_types::WorkspaceEdit> {
    let lsp = match lang.lsp.as_ref() {
        Some(lsp) => lsp,
        None => bail!("rename symbol not supported"),
    };

    trace!("rename_symbol: {}", path.display());
    let fut = client.rename_symbol(lsp, &path, pos.into(), new_name);
    match timeout(Duration::from_secs(5), fut).await {
        Ok(Ok(edit)) => Ok(edit),
        Ok(Err(err)) => {
            bail!("LSP rename symbol failed: {}", err);
        }
        Err(_) => {
            bail!("LSP rename symbol timed out");
        }
    }
}
