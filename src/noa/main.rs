use noa_common::{dirs::backup_dir, logger::install_logger, syncd_protocol::LspRequest};
use parking_lot::RwLock;
use std::{
    env::current_dir,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use structopt::StructOpt;
use surfaces::CompletionSurface;
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedSender},
        Mutex,
    },
    time::timeout,
};
use ui::Event;

use crate::{
    editor::OpenedFile,
    surfaces::{BottomBarSurface, BufferSurface},
    syncd_client::SyncdClient,
    terminal::Terminal,
    ui::Compositor,
    ui::{Context, DEFAULT_THEME},
};

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

mod editor;
mod fuzzy_set;
mod line_edit;
mod open;
mod selector;
mod surfaces;
mod syncd_client;
mod terminal;
mod ui;
mod view;

#[derive(StructOpt)]
struct Opt {
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
    #[structopt(long)]
    open_path_in_tmux: bool,
    #[structopt(long)]
    tmux_pane: Option<String>,
    #[structopt(long)]
    tmux_mouse_y: Option<usize>,
    #[structopt(long)]
    tmux_mouse_x: Option<usize>,
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();

    if opt.open_path_in_tmux {
        open::open_path_in_tmux(
            opt.tmux_pane.expect("--tmux-pane is required").as_str(),
            opt.tmux_mouse_y.expect("--tmux-mouse-y is required"),
            opt.tmux_mouse_x.expect("--tmux-mouse-x is required"),
        );
        return;
    }

    install_logger();

    let workspace_dir = match opt.files.get(0) {
        Some(file_or_dir) if file_or_dir.is_dir() => file_or_dir.clone(),
        _ => current_dir().unwrap(),
    };

    // Initialize editor components.
    let mut editor = editor::Editor::new(&workspace_dir);
    let (event_tx, mut event_rx) = unbounded_channel();

    let theme = DEFAULT_THEME;
    let mut ctx = Context {
        editor: &mut editor,
        event_tx: &event_tx,
        theme,
    };

    // Initialize UI.
    let terminal = Terminal::new(event_tx.clone());
    let mut compositor = Compositor::new(terminal);
    let completion = CompletionSurface::new(&mut ctx);
    compositor.push_layer(&mut ctx, BufferSurface::new());
    compositor.push_layer(&mut ctx, BottomBarSurface::new());
    compositor.push_layer(&mut ctx, completion);

    // Open speicifed file or the workspace dir.
    for file in opt.files.iter() {
        if !file.is_file() {
            continue;
        }

        editor.open_file(file);
    }

    tokio::spawn(update_highlight(
        editor.current_file().clone(),
        event_tx.clone(),
    ));

    // The main event loop.
    let backup_dir = backup_dir();
    let mut updated = false;
    while !editor.exited() {
        let mut ctx = Context {
            editor: &mut editor,
            event_tx: &event_tx,
            theme,
        };

        if updated {
            compositor.render_to_terminal(&mut ctx);
        }

        match timeout(Duration::from_millis(400), event_rx.recv()).await {
            Ok(Some(ev)) => {
                let started_at = Instant::now();
                let prev_ver = editor.current_file().read().buffer.id_and_version();

                let mut ctx = Context {
                    editor: &mut editor,
                    event_tx: &event_tx,
                    theme,
                };
                compositor.handle_event(&mut ctx, ev);
                while let Ok(Some(ev)) = timeout(Duration::from_micros(400), event_rx.recv()).await
                {
                    compositor.handle_event(&mut ctx, ev);
                }

                let new_ver = editor.current_file().read().buffer.id_and_version();
                if prev_ver != new_ver {
                    // Switched or modified the current buffer.
                    tokio::spawn(update_highlight(
                        editor.current_file().clone(),
                        event_tx.clone(),
                    ));
                    tokio::spawn(sync_file_with_lsp(
                        editor.current_file().clone(),
                        workspace_dir.clone(),
                        editor.syncd().clone(),
                    ));
                }

                updated = true;
                trace!(
                    "event handling took {} us",
                    started_at.elapsed().as_micros()
                );
            }
            Ok(None) => {
                break;
            }
            Err(_) if updated => {
                // Idle.

                {
                    let f = editor.current_file().read();
                    f.buffer.update_backup(&backup_dir);
                }

                {
                    let mut f = editor.current_file().write();
                    f.buffer.mark_undo_point();
                }

                updated = false;
            }
            Err(_) => {}
        }
    }

    trace!("exiting the editor");
}

async fn update_highlight(switch_to: Arc<RwLock<OpenedFile>>, tx: UnboundedSender<Event>) {
    let (rope, mut parser) = {
        let f = switch_to.read();
        let rope = f.buffer.rope().clone();
        let parser = match f.buffer.lang().syntax_highlighting_parser() {
            Some(parser) => parser,
            None => return,
        };
        (rope, parser)
    };

    if let Some(tree) = parser.parse(rope.text(), None) {
        switch_to.write().syntax_highlight = Some(tree);
    }

    tx.send(Event::ReDraw).ok();
}

async fn sync_file_with_lsp(
    opened_file: Arc<RwLock<OpenedFile>>,
    workspace_dir: PathBuf,
    syncd: Arc<Mutex<SyncdClient>>,
) {
    let (lang, file_modified_req) = {
        let opend_file = opened_file.read();
        match opend_file.buffer.path_for_lsp(&workspace_dir) {
            Some(path) => (
                opend_file.buffer.lang(),
                LspRequest::UpdateFile {
                    path,
                    version: opend_file.buffer.version(),
                    text: opend_file.buffer.text(),
                },
            ),
            None => return,
        }
    };

    if let Err(err) = syncd
        .lock()
        .await
        .call_lsp_method(lang, file_modified_req)
        .await
    {
        warn!("failed to send UpdateFile request: {}", err);
    }
}
