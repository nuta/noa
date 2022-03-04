use std::sync::Arc;

use fuzzy_matcher::skim::SkimMatcherV2;
use noa_buffer::{
    cursor::{Cursor, Position},
    raw_buffer::RawBuffer,
};
use noa_common::prioritized_vec::PrioritizedVec;
use noa_proxy::client::Client as ProxyClient;
use tokio::sync::oneshot;

use crate::document::Words;

pub fn build_fuzzy_matcher() -> SkimMatcherV2 {
    SkimMatcherV2::default().smart_case().use_cache(true)
}

/// This should be called after `Document::post_update_job` because the buffer
/// needs to be synced with the LSP server before querying the completion.
pub fn complete(proxy: Arc<ProxyClient>, buffer: RawBuffer, main_cursor: Cursor, words: Words) {
    const NUM_ITEMS_MAX: usize = 16;

    // Send the LSP request in background becuase it would take a time.
    let (lsp_items_tx, lsp_items_rx) = oneshot::channel();
    tokio::spawn({
        let lang = todo!();
        let path = todo!();
        let position = todo!();
        async move {
            if let Ok(lsp_items) = proxy.completion(lang, path, position).await {
                lsp_items_tx.send(lsp_items).unwrap();
            }
        }
    });

    tokio::spawn(async move {
        // Any word comopletion.
        let mut items = PrioritizedVec::with_max_capacity(NUM_ITEMS_MAX);
        if !main_cursor.is_selection() {
            let pos = main_cursor.moving_position();
            if let Some(current_word_range) = buffer.current_word(pos) {
                let current_word = buffer.substr(current_word_range);
                items.extend(words.search(&current_word, NUM_ITEMS_MAX));
            }
        }

        // Wait for the response from the LSP server.
        if let Ok(lsp_items) = lsp_items_rx.await {
            // items.extend(lsp_items);
        }

        // TODO: Apply changes
    });
}
