use log::LevelFilter;
use noa_common::{
    dirs::{backup_dir, log_file_path},
    syncd_protocol::LspRequest,
};
use parking_lot::RwLock;
use simplelog::{Config, WriteLogger};
use std::{
    env::current_dir,
    fs::OpenOptions,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use structopt::StructOpt;
use surfaces::CompletionSurface;
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver},
        Mutex,
    },
    time::timeout,
};

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
}

#[tokio::main]
async fn main() {
    WriteLogger::init(
        LevelFilter::Trace,
        Config::default(),
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_file_path("noa"))
            .unwrap(),
    )
    .unwrap();

    std::panic::set_hook(Box::new(|info| {
        error!("{}", info);
        let backtrace = backtrace::Backtrace::new();
        for (i, frame) in backtrace.frames().iter().enumerate() {
            for symbol in frame.symbols() {
                if let Some(path) = symbol.filename() {
                    let filename = path.to_str().unwrap_or("(non-utf8 path)");
                    if filename.contains("/.rustup/")
                        || filename.contains("/.cargo/")
                        || filename.starts_with("/rustc/")
                    {
                        continue;
                    }

                    error!(
                        "    #{} {}:{}, col {}",
                        i,
                        filename,
                        symbol.lineno().unwrap_or(0),
                        symbol.colno().unwrap_or(0),
                    );
                }
            }
        }
    }));

    let opt = Opt::from_args();
    let workspace_dir = match opt.files.get(0) {
        Some(file_or_dir) if file_or_dir.is_dir() => file_or_dir.clone(),
        _ => current_dir().unwrap(),
    };

    // Initialize editor components.
    let mut editor = editor::Editor::new(workspace_dir);
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

    // Register the event handler on file updates.
    let (file_updated_tx, file_updated_rx) = unbounded_channel::<Arc<RwLock<OpenedFile>>>();
    tokio::spawn(on_file_change(
        file_updated_rx,
        editor.workspace_dir().to_path_buf(),
        editor.syncd().clone(),
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
                    file_updated_tx.send(editor.current_file().clone()).ok();
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
                let mut opened_file = editor.current_file().write();
                opened_file.buffer.update_backup(&backup_dir);
                opened_file.buffer.mark_undo_point();
                updated = false;
            }
            Err(_) => {}
        }
    }

    trace!("exiting the editor");
}

async fn on_file_change(
    mut rx: UnboundedReceiver<Arc<RwLock<OpenedFile>>>,
    workspace_dir: PathBuf,
    syncd: Arc<Mutex<SyncdClient>>,
) {
    while let Some(opend_file) = rx.recv().await {
        let (lang, file_modified_req) = {
            let opend_file = opend_file.read();
            match opend_file.buffer.path_for_lsp(&workspace_dir) {
                Some(path) => (
                    opend_file.buffer.lang(),
                    LspRequest::UpdateFile {
                        path,
                        version: opend_file.buffer.version(),
                        text: opend_file.buffer.text(),
                    },
                ),
                None => continue,
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

    trace!("exiting file update handler");
}
