use std::{
    cmp::{max, min},
    path::PathBuf,
    sync::Arc,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use noa_common::{
    dirs::noa_bin_args,
    oops::OopsExt,
    tmux::{self, in_tmux},
};
use parking_lot::Mutex;
use tokio::{process::Command, sync::mpsc::UnboundedSender};

use crate::{
    fuzzy_set::FuzzySet,
    line_edit::LineEdit,
    selector::Selector,
    ui::{
        CanvasViewMut, Compositor, Context, Decoration, Event, HandledEvent, Layout, RectSize,
        Surface,
    },
};

#[derive(Debug)]
enum Item {
    File(PathBuf),
}

pub struct FinderSurface {
    input: LineEdit,
    selector: Arc<Mutex<Selector<Item>>>,
}

impl FinderSurface {
    pub fn new(ctx: &mut Context) -> FinderSurface {
        let selector = Arc::new(Mutex::new(Selector::new()));

        tokio::spawn(update_items(
            ctx.editor.workspace_dir().to_owned(),
            ctx.event_tx.clone(),
            selector.clone(),
            "".to_owned(),
        ));

        FinderSurface {
            input: LineEdit::new(),
            selector,
        }
    }

    fn select(&self, ctx: &mut Context, item: &Item) {
        match item {
            Item::File(path) => {
                ctx.editor.open_file(path, None);
            }
        }
    }

    fn select_in_new_pane(&self, ctx: &mut Context, item: &Item) {
        if !in_tmux() {
            ctx.editor.error("not in tmux");
            return;
        }

        match item {
            Item::File(path) => {
                ctx.editor
                    .info(format!("opening {} in new pane", path.display()));
                ctx.editor.check_run_background(
                    "tmux-splitw",
                    Command::new("tmux")
                        .args(&["splitw", "-h"])
                        .args(noa_bin_args())
                        .arg(path),
                );
            }
        }
    }

    fn select_in_other_pane(&self, ctx: &mut Context, item: &Item) {
        if !in_tmux() {
            ctx.editor.error("not in tmux");
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

                ctx.editor
                    .info(format!("opening {} in other pane", path.display(),));

                let sync = ctx.editor.sync().clone();
                let path = path.to_owned();
                tokio::spawn(async move {
                    sync.lock()
                        .await
                        .call_buffer_open_file_in_other(&pane_id, &path, None)
                        .await
                        .oops();
                });
            }
        }
    }
}

impl Surface for FinderSurface {
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

    fn render<'a>(&mut self, _ctx: &mut Context, mut canvas: CanvasViewMut<'a>) {
        canvas.clear();
        let mut inner = canvas.draw_borders(0, 0, canvas.height(), canvas.width());

        inner.draw_str(0, 0, &self.input.text());

        let selector = self.selector.lock();
        for (i, (active, item)) in selector.items().take(inner.height() - 1).enumerate() {
            let title = match item {
                Item::File(path) => path.to_str().unwrap(),
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

    fn handle_key_event(
        &mut self,
        ctx: &mut Context,
        compositor: &mut Compositor,
        key: KeyEvent,
    ) -> HandledEvent {
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
                        self.select(ctx, item);
                    }

                    compositor.pop_layer();
                    return HandledEvent::Consumed;
                }
                (KeyModifiers::ALT, KeyCode::Enter) => {
                    if let Some(item) = self.selector.lock().selected() {
                        self.select_in_new_pane(ctx, item);
                    }

                    compositor.pop_layer();
                    return HandledEvent::Consumed;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('o')) => {
                    if let Some(item) = self.selector.lock().selected() {
                        self.select_in_other_pane(ctx, item);
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
                ctx.editor.workspace_dir().to_owned(),
                ctx.event_tx.clone(),
                self.selector.clone(),
                self.input.text(),
            ));
        }
        HandledEvent::Consumed
    }

    fn handle_key_batch_event(
        &mut self,
        _ctx: &mut Context,
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
    let path_resolver = async move {
        let results = Mutex::new(FuzzySet::with_capacity(32));
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
                                / max(1, min(elapsed.as_secs() / (3600 * 24 * 30), 3600 * 24 * 30)))
                                as isize;
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
        results
    };

    // Merge results.
    let iter = futures::future::join_all(vec![path_resolver]).await;
    let mut selector = selector.lock();
    selector.clear();
    for results in iter {
        for item in results.into_inner().into_vec() {
            selector.push(item.value);
        }
    }

    event_tx.send(Event::ReDraw).unwrap();
}
