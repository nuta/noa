use std::{cmp::max, sync::Arc};

use noa_buffer::Snapshot;
use noa_cui::truncate_to_width;
use noa_cui::{KeyCode, KeyEvent, KeyModifiers};
use parking_lot::{Mutex, RwLock};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    buffer_set::{BufferSet, OpenedFile},
    fuzzy_set::FuzzySet,
    selector::Selector,
    sync_client::SyncClient,
    Event,
};

use noa_cui::{
    CanvasViewMut, Compositor, Decoration, DisplayWidth, HandledEvent, Layout, RectSize, Surface,
};

const MIN_WIDTH: usize = 16;

enum Item {
    Word {
        display_text: String,
        insert_text: Option<String>,
    },
}

pub struct Completion {
    buffers: Arc<RwLock<BufferSet>>,
    event_tx: UnboundedSender<Event>,
    sync: Arc<SyncClient>,
    selector: Arc<Mutex<Selector<Item>>>,
}

impl Completion {
    pub fn new(
        buffers: Arc<RwLock<BufferSet>>,
        event_tx: UnboundedSender<Event>,
        sync: Arc<SyncClient>,
    ) -> Completion {
        let selector = Arc::new(Mutex::new(Selector::new()));
        let (current_file, current_word, snapshot) = {
            let buffers = buffers.read();
            let current_file = buffers.current_file().clone();
            let f = buffers.current_file().read();
            let current_word = f.buffer.current_word().unwrap_or_else(|| "".to_owned());
            let snapshot = f.buffer.take_snapshot();
            (current_file, current_word, snapshot)
        };

        tokio::spawn(update_completion(
            event_tx.clone(),
            selector,
            current_word,
            sync.clone(),
            current_file,
            snapshot,
        ));

        Completion {
            buffers,
            event_tx,
            sync,
            selector: Arc::new(Mutex::new(Selector::new())),
        }
    }
}

impl Surface for Completion {
    fn name(&self) -> &str {
        "completion"
    }

    fn is_visible(&self) -> bool {
        !self.selector.lock().is_empty()
    }

    fn layout(&self, _screen_size: RectSize) -> (Layout, RectSize) {
        let selector = self.selector.lock();

        // Determine the maximum item width.
        let max_width = selector
            .items()
            .take(16)
            .fold(MIN_WIDTH, |max_width, (_, item)| {
                let width = match item {
                    Item::Word { display_text, .. } => display_text.display_width(),
                };

                max(max_width, width)
            });

        let width = max_width + 2 /* border */;
        let height = selector.len() + 2 /* border */;
        (Layout::AroundCursor, RectSize { height, width })
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        None
    }

    fn render<'a>(&mut self, mut canvas: CanvasViewMut<'a>) {
        canvas.clear();
        canvas.draw_borders(0, 0, canvas.height() - 1, canvas.width() - 1);

        for (i, (active, item)) in self
            .selector
            .lock()
            .items()
            .take(canvas.height().saturating_sub(2))
            .enumerate()
        {
            let text = match item {
                Item::Word { display_text, .. } => display_text,
            };

            let y = 1 + i;
            let x = 1;
            canvas.draw_str(y, x, truncate_to_width(text, canvas.width() - 1));

            if active {
                canvas.set_deco(
                    y,
                    x,
                    canvas.width() - 1,
                    Decoration {
                        bold: true,
                        underline: true,
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn handle_key_event(&mut self, _compositor: &mut Compositor, key: KeyEvent) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        // const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        if matches!(key.code, KeyCode::Char(_)) {
            let buffers = self.buffers.read();
            let current_file = buffers.current_file();
            let opened_file = current_file.read();
            let current_word = opened_file
                .buffer
                .current_word()
                .unwrap_or_else(|| "".to_owned());
            let snapshot = opened_file.buffer.take_snapshot();

            tokio::spawn(update_completion(
                self.event_tx.clone(),
                self.selector.clone(),
                current_word,
                self.sync.clone(),
                current_file.clone(),
                snapshot,
            ));
        }

        let mut selector = self.selector.lock();
        if selector.is_empty() {
            return HandledEvent::Ignored;
        }

        match (key.modifiers, key.code) {
            (NONE, KeyCode::Esc) => {
                selector.clear();
                HandledEvent::Consumed
            }
            (NONE, KeyCode::Up) => {
                selector.select_prev();
                HandledEvent::Consumed
            }
            (NONE, KeyCode::Down) => {
                selector.select_next();
                HandledEvent::Consumed
            }
            (NONE, KeyCode::Enter) => {
                if let Some(selected) = selector.selected() {
                    match selected {
                        Item::Word {
                            insert_text,
                            display_text,
                        } => {
                            let buffers = self.buffers.read();
                            let mut f = buffers.current_file().write();
                            if let Some(range) = f.buffer.current_word_range() {
                                f.buffer.select_by_ranges(&[range]);
                                f.buffer.insert(match insert_text {
                                    Some(insert_text) => insert_text,
                                    None => display_text,
                                });
                            }
                        }
                    }
                }

                selector.clear();
                HandledEvent::Consumed
            }
            _ => HandledEvent::Ignored,
        }
    }

    fn handle_key_batch_event(
        &mut self,

        _compositor: &mut Compositor,
        _input: &str,
    ) -> HandledEvent {
        HandledEvent::Ignored
    }
}

async fn update_completion(
    event_tx: UnboundedSender<Event>,
    selector: Arc<Mutex<Selector<Item>>>,
    query: String,
    sync: Arc<SyncClient>,
    opened_file: Arc<RwLock<OpenedFile>>,
    snapshot: Arc<Snapshot>,
) {
    // Word completion.
    let word_comp = async move {
        let mut results = FuzzySet::with_capacity(32);
        for word in snapshot.words() {
            if word == query {
                continue;
            }

            if let Some(m) = sublime_fuzzy::best_match(&query, word) {
                results.push(
                    m.score(),
                    Item::Word {
                        display_text: word.to_owned(),
                        insert_text: None,
                    },
                );
            }
        }

        results
    };

    // LSP completion.
    let lsp_comp = async move {
        let mut results = FuzzySet::with_capacity(32);
        trace!("sending completion message...");

        match sync.call_completion(&opened_file).await {
            Ok(result) => {
                if let Ok(items) = result.await {
                    let mut score = items.len() as isize;
                    for item in items {
                        results.push(
                            score,
                            Item::Word {
                                display_text: item.label,
                                insert_text: item.insert_text,
                            },
                        );
                        score -= 1;
                    }
                }
            }
            Err(err) => {
                warn!("failed to call Completion request: {}", err);
            }
        }
        results
    };

    // Merge results.
    let (word_comp_iter, lsp_comp_iter) = futures::join!(word_comp, lsp_comp);
    let mut selector = selector.lock();
    selector.clear();

    for item in word_comp_iter.into_vec() {
        selector.push(item.value);
    }

    for item in lsp_comp_iter.into_vec() {
        selector.push(item.value);
    }

    event_tx.send(Event::ReDraw).unwrap();
}
