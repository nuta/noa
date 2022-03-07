use std::{path::PathBuf, sync::Arc};

use fuzzy_matcher::skim::SkimMatcherV2;
use noa_buffer::{
    buffer::Buffer,
    cursor::{Cursor, Position, Range},
    raw_buffer::RawBuffer,
};
use noa_common::prioritized_vec::PrioritizedVec;
use noa_languages::language::Language;
use noa_proxy::{
    client::Client as ProxyClient,
    lsp_types::{self, CompletionTextEdit},
};
use tokio::sync::oneshot;

use crate::document::Words;

pub fn build_fuzzy_matcher() -> SkimMatcherV2 {
    SkimMatcherV2::default().smart_case().use_cache(true)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CompletionKind {
    AnyWord,
    LspItem,
}

#[derive(Clone, PartialEq, Debug)]
pub struct CompletionItem {
    pub kind: CompletionKind,
    pub range: Range,
    pub insert_text: String,
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

    let pos = main_cursor.moving_position().into();
    let current_word_range = match buffer.current_word(pos) {
        Some(range) => range,
        None => return None,
    };

    // Send the LSP request in background becuase it would take a time.
    let (lsp_items_tx, lsp_items_rx) = oneshot::channel();
    tokio::spawn(async move {
        if let Ok(lsp_items) = proxy.completion(lang, &path, pos.into()).await {
            lsp_items_tx.send(lsp_items).unwrap();
        }
    });

    // Any word comopletion.
    let mut items = Vec::new();
    let current_word = buffer.substr(current_word_range);
    items.extend(
        words
            .search(&current_word, NUM_ITEMS_MAX)
            .into_sorted_vec()
            .drain(..)
            .map(|word| CompletionItem {
                kind: CompletionKind::AnyWord,
                range: current_word_range,
                insert_text: word,
            }),
    );

    // Wait for the response from the LSP server.
    if let Ok(mut lsp_items) = lsp_items_rx.await {
        for lsp_item in lsp_items.drain(..) {
            let item = match (&lsp_item.insert_text, &lsp_item.text_edit) {
                (Some(insert_text), None) => CompletionItem {
                    kind: CompletionKind::LspItem,
                    range: current_word_range,
                    insert_text: insert_text.to_owned(),
                },
                (None, Some(CompletionTextEdit::Edit(edit))) => CompletionItem {
                    kind: CompletionKind::LspItem,
                    range: edit.range.into(),
                    insert_text: edit.new_text.to_owned(),
                },
                _ => {
                    warn!("unsupported LSP completion item: {:?}", lsp_item);
                    continue;
                }
            };

            items.push(item);
        }
    }

    Some(items)
}
