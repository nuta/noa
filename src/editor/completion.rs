use std::{path::PathBuf, sync::Arc};

use fuzzy_matcher::skim::SkimMatcherV2;
use noa_buffer::{buffer::TextEdit, cursor::Cursor, raw_buffer::RawBuffer};

use noa_compositor::Compositor;
use noa_languages::language::Language;
use noa_proxy::{client::Client as ProxyClient, lsp_types::CompletionTextEdit};
use tokio::sync::oneshot;

use crate::{
    document::{Document, Words},
    editor::Editor,
    ui::completion_view::CompletionView,
};

pub fn build_fuzzy_matcher() -> SkimMatcherV2 {
    SkimMatcherV2::default().smart_case().use_cache(true)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CompletionKind {
    AnyWord,
    LspItem,
}

#[derive(Clone, Debug)]
pub struct CompletionItem {
    pub kind: CompletionKind,
    pub label: String,
    pub text_edits: Vec<TextEdit>,
}

/// This should be called after `Document::post_update_job` because the buffer
/// needs to be synced with the LSP server before querying the completion.
pub async fn complete(
    proxy: Arc<ProxyClient>,
    buffer: RawBuffer,
    lang: &'static Language,
    path: PathBuf,
    main_cursor: Cursor,
    words: Words,
) -> Option<Vec<CompletionItem>> {
    const NUM_ITEMS_MAX: usize = 16;

    if main_cursor.is_selection() {
        return None;
    }

    let pos = main_cursor.moving_position();
    let current_word_range = match buffer.current_word(pos) {
        Some(range) => range,
        None => return None,
    };

    // Send the LSP request in background becuase it would take a time.
    let (lsp_items_tx, lsp_items_rx) = oneshot::channel();
    if let Some(lsp) = lang.lsp.as_ref() {
        tokio::spawn(async move {
            if let Ok(lsp_items) = proxy.completion(lsp, &path, pos.into()).await {
                lsp_items_tx.send(lsp_items).unwrap();
            }
        });
    }

    // Any word comopletion.
    let mut items = Vec::new();
    let current_word = buffer.substr(current_word_range);
    if current_word.len() >= 3 {
        items.extend(
            words
                .search(&current_word, NUM_ITEMS_MAX)
                .into_sorted_vec()
                .drain(..)
                .filter(|word| word != &current_word)
                .map(|word| CompletionItem {
                    kind: CompletionKind::AnyWord,
                    label: word.clone(),
                    text_edits: vec![TextEdit {
                        range: current_word_range,
                        new_text: word,
                    }],
                }),
        );
    }

    // Wait for the response from the LSP server.
    if let Ok(mut lsp_items) = lsp_items_rx.await {
        for lsp_item in lsp_items.drain(..) {
            let mut text_edits: Vec<TextEdit> = lsp_item
                .additional_text_edits
                .clone()
                .unwrap_or_default()
                .drain(..)
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
    }

    // Make items unique.
    let mut unique_items: Vec<CompletionItem> = Vec::with_capacity(items.len());
    for item in items {
        if unique_items.iter().all(|i| i.text_edits != item.text_edits) {
            unique_items.push(item);
        }
    }

    Some(unique_items)
}

pub fn clear_completion(doc: &mut Document, compositor: &mut Compositor<Editor>) {
    compositor
        .get_mut_surface_by_name::<CompletionView>("completion")
        .set_active(false);

    doc.clear_completion_items();
}
