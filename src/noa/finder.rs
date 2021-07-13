use std::{
    cmp::{max, min},
    path::PathBuf,
    sync::Arc,
};

use noa_common::{
    dirs::noa_bin_args,
    oops::OopsExt,
    tmux::{self, in_tmux},
};
use noa_cui::{KeyCode, KeyEvent, KeyModifiers, LineEdit};
use parking_lot::{Mutex, RwLock};
use tokio::{process::Command, sync::mpsc::UnboundedSender};

use crate::{
    actions::{self, Action, ACTIONS},
    buffer_set::BufferSet,
    fuzzy_set::FuzzySet,
    selector::Selector,
    sync_client::SyncClient,
    textarea::StatusBar,
    Event,
};

use noa_cui::{CanvasViewMut, Compositor, Decoration, HandledEvent, Layout, RectSize, Surface};

#[derive(Debug)]
enum Item {
    File(PathBuf),
    Action(Arc<dyn Action>),
}

pub struct Finder {
    status_bar: Arc<StatusBar>,
    buffers: Arc<RwLock<BufferSet>>,
    workspace_dir: PathBuf,
    event_tx: UnboundedSender<Event>,
    input: LineEdit,
    selector: Arc<Mutex<Selector<Item>>>,
    sync: Arc<SyncClient>,
}

impl Finder {
    pub fn new(
        status_bar: Arc<StatusBar>,
        buffers: Arc<RwLock<BufferSet>>,
        workspace_dir: PathBuf,
        event_tx: UnboundedSender<Event>,
        sync: Arc<SyncClient>,
        initial_input: Option<&str>,
    ) -> Finder {
        let selector = Arc::new(Mutex::new(Selector::new()));

        let query = initial_input.unwrap_or("");
        tokio::spawn(update_items(
            workspace_dir.to_owned(),
            event_tx.clone(),
            selector.clone(),
            query.to_owned(),
        ));

        Finder {
            status_bar,
            buffers,
            workspace_dir,
            event_tx,
            input: LineEdit::from_str(query),
            selector,
            sync,
        }
    }

    fn select(&self, _compositor: &mut Compositor, item: &Item) {
        match item {
            Item::File(path) => {
                self.status_bar
                    .report_if_error(self.buffers.write().open_file(
                        &self.sync,
                        &self.event_tx,
                        path,
                        None,
                    ));
            }
            Item::Action(action) => {
                action.execute(&actions::Context {
                    buffers: &self.buffers,
                });
            }
        }
    }

    fn select_in_new_pane(&self, item: &Item) {
        if !in_tmux() {
            self.status_bar.error("not in tmux");
            return;
        }

        match item {
            Item::File(path) => {
                self.status_bar
                    .info(format!("opening {} in new pane", path.display()));
                self.status_bar.check_run_background(
                    "tmux-splitw",
                    Command::new("tmux")
                        .args(&["splitw", "-h"])
                        .args(noa_bin_args())
                        .arg(path),
                );
            }
            Item::Action(_) => {}
        }
    }

    fn select_in_other_pane(&self, item: &Item) {
        if !in_tmux() {
            self.status_bar.error("not in tmux");
            return;
        }

        match item {
            Item::File(path) => {
                let pane_id = match tmux::get_other_noa_pane_id() {
                    Ok(pane_id) => pane_id,
                    Err(err) => {
                        warn!("failed to open in other pane: {:?}", err);
                        return;
                    }
                };

                self.status_bar
                    .info(format!("opening {} in other pane", path.display(),));

                let sync = self.sync.clone();
                let path = path.to_owned();
                tokio::spawn(async move {
                    sync.call_buffer_open_file_in_other(&pane_id, &path, None)
                        .await
                        .oops();
                });
            }
            Item::Action(_) => {}
        }
    }
}

impl Surface for Finder {
    fn name(&self) -> &str {
        "finder"
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn layout(&self, screen_size: RectSize) -> (Layout, RectSize) {
        let rect_size = RectSize {
            width: min(max(screen_size.width, 32), 80),
            height: min(max(screen_size.height, 8), 16),
        };
        (Layout::Center, rect_size)
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        Some((1, 1 + self.input.cursor_display_pos()))
    }

    fn render<'a>(&mut self, mut canvas: CanvasViewMut<'a>) {
        canvas.clear();
        let mut inner = canvas.draw_borders(0, 0, canvas.height(), canvas.width());

        inner.draw_str(0, 0, &self.input.text());

        let selector = self.selector.lock();
        for (i, (active, item)) in selector.items().take(inner.height() - 1).enumerate() {
            let title = match item {
                Item::File(path) => path.to_str().unwrap(),
                Item::Action(action) => action.title(),
            };

            inner.draw_str(1 + i, 0, title);
            if active {
                inner.set_deco(
                    1 + i,
                    1,
                    inner.width(),
                    Decoration {
                        bold: true,
                        underline: true,
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn handle_key_event(&mut self, compositor: &mut Compositor, key: KeyEvent) -> HandledEvent {
        const NONE: KeyModifiers = KeyModifiers::NONE;
        // const CTRL: KeyModifiers = KeyModifiers::CONTROL;
        // const ALT: KeyModifiers = KeyModifiers::ALT;
        // const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

        let prev_query = self.input.rope().clone();
        if !self.input.consume_key_event(key) {
            info!("key: {:?} {:?}", key.modifiers, key.code);
            match (key.modifiers, key.code) {
                (NONE, KeyCode::Esc) => {
                    compositor.pop_layer();
                    return HandledEvent::Consumed;
                }
                (NONE, KeyCode::Up) => {
                    self.selector.lock().select_prev();
                }
                (NONE, KeyCode::Down) => {
                    self.selector.lock().select_next();
                }
                (NONE, KeyCode::Enter) => {
                    if let Some(item) = self.selector.lock().selected() {
                        self.select(compositor, item);
                    }

                    compositor.pop_layer();
                    return HandledEvent::Consumed;
                }
                (KeyModifiers::ALT, KeyCode::Enter) => {
                    if let Some(item) = self.selector.lock().selected() {
                        self.select_in_new_pane(item);
                    }

                    compositor.pop_layer();
                    return HandledEvent::Consumed;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('o')) => {
                    if let Some(item) = self.selector.lock().selected() {
                        self.select_in_other_pane(item);
                    }

                    compositor.pop_layer();
                    return HandledEvent::Consumed;
                }
                _ => {
                    trace!("finder: unhandled key event: {:?}", key);
                    return HandledEvent::Consumed;
                }
            }
        }

        if &prev_query != self.input.rope() {
            tokio::spawn(update_items(
                self.workspace_dir.clone(),
                self.event_tx.clone(),
                self.selector.clone(),
                self.input.text(),
            ));
        }
        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,

        _compositor: &mut Compositor,
        input: &str,
    ) -> HandledEvent {
        self.input.insert(input);
        HandledEvent::Consumed
    }
}

async fn update_items(
    workspace_dir: PathBuf,
    event_tx: UnboundedSender<Event>,
    selector: Arc<Mutex<Selector<Item>>>,
    query: String,
) {
    use ignore::{WalkBuilder, WalkState};

    // Scan all files.
    let path_resolver;
    {
        let query = query.clone();
        path_resolver = tokio::spawn(async move {
            let results = Mutex::new(FuzzySet::with_capacity(32));
            if !query.starts_with('>') {
                WalkBuilder::new(workspace_dir).build_parallel().run(|| {
                    Box::new(|dirent| {
                        if let Ok(dirent) = dirent {
                            let meta = dirent.metadata().unwrap();
                            if !meta.is_file() {
                                return WalkState::Continue;
                            }

                            let path = dirent.path().to_str().unwrap();
                            let mut score = 0;

                            // Fuzzy match.
                            if let Some(m) = sublime_fuzzy::best_match(&query, path) {
                                score += m.score();
                            }

                            // Recently used.
                            if let Ok(atime) = meta.accessed() {
                                if let Ok(elapsed) = atime.elapsed() {
                                    score += (100 / max(1, min(elapsed.as_secs(), 360))) as isize;
                                    score += (100 / max(1, elapsed.as_secs())) as isize;
                                }
                            }

                            if let Ok(mtime) = meta.modified() {
                                if let Ok(elapsed) = mtime.elapsed() {
                                    score += (10
                                        / max(
                                            1,
                                            min(
                                                elapsed.as_secs() / (3600 * 24 * 30),
                                                3600 * 24 * 30,
                                            ),
                                        )) as isize;
                                    score += (100 / max(1, min(elapsed.as_secs(), 360))) as isize;
                                    score += (100 / max(1, elapsed.as_secs())) as isize;
                                }
                            }

                            if score != 0 {
                                results
                                    .lock()
                                    .push(score, Item::File(dirent.path().to_owned()));
                            }
                        }
                        WalkState::Continue
                    })
                });
            }

            results
        });
    }

    // Actions.
    let actions_resolver = tokio::spawn(async move {
        let query = query.clone();
        let results = Mutex::new(FuzzySet::with_capacity(ACTIONS.len()));
        if query.starts_with('>') {
            let mut results = results.lock();
            for action in ACTIONS.values() {
                let mut score = 0;

                // Fuzzy match.
                if let Some(m) = sublime_fuzzy::best_match(&query, action.title()) {
                    score += m.score();
                }

                results.push(score, Item::Action(action.clone()));
            }
        }
        results
    });

    // Merge results.
    let iter = futures::future::join_all(vec![path_resolver, actions_resolver]).await;
    let mut selector = selector.lock();
    selector.clear();
    for results in iter {
        for item in results.unwrap().into_inner().into_vec() {
            selector.push(item.value);
        }
    }

    event_tx.send(Event::ReDraw).unwrap();
}
