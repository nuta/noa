use std::{path::PathBuf, sync::Arc};

use fuzzy_matcher::skim::SkimMatcherV2;
use noa_buffer::{buffer::TextEdit, cursor::Cursor, raw_buffer::RawBuffer};

use noa_compositor::Compositor;
use noa_languages::Language;
use noa_proxy::client::Client as ProxyClient;

use crate::{
    document::{Document, Words},
    editor::Editor,
    lsp,
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

    // Start sending a LSP request in background.
    let lsp_items = tokio::spawn(lsp::completion_hook(
        lang,
        proxy.clone(),
        path.to_owned(),
        pos,
        current_word_range,
    ));

    // Any word comopletion.
    let mut items = Vec::new();
    let current_word = buffer.substr(current_word_range);
    if current_word.len() >= 3 {
        items.extend(
            words
                .search(&current_word, NUM_ITEMS_MAX)
                .into_sorted_vec()
                .into_iter()
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
    match lsp_items.await {
        Ok(Ok(lsp_items)) => {
            items.extend(lsp_items);
        }
        Ok(Err(err)) => {
            warn!("failed to get LSP completion: {}", err);
        }
        Err(err) => {
            warn!("failed to join LSP completion task: {:?}", err);
        }
    };

    // Make items unique.
    let mut unique_items: Vec<CompletionItem> = Vec::with_capacity(items.len());
    for item in items {
        if unique_items.iter().all(|i| i.text_edits != item.text_edits) {
            unique_items.push(item);
        }
    }

    Some(unique_items)
}

pub fn clear_completion(compositor: &mut Compositor<Editor>, doc: &mut Document) {
    compositor
        .get_mut_surface_by_name::<CompletionView>("completion")
        .set_active(false);

    doc.clear_completion_items();
}
