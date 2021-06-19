use log::LevelFilter;
use noa_buffer::Buffer;
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
    surfaces::BufferSurface, syncd_client::SyncdClient, terminal::Terminal, ui::Compositor,
    ui::Context,
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

    let mut editor = editor::Editor::new(workspace_dir);

    let (event_tx, mut event_rx) = unbounded_channel();
    let terminal = Terminal::new(event_tx.clone());
    let mut compositor = Compositor::new(terminal);

    let mut ctx = Context {
        editor: &mut editor,
        event_tx: &event_tx,
    };

    let completion = CompletionSurface::new(&mut ctx);
    compositor.push_layer(&mut ctx, BufferSurface::new());
    compositor.push_layer(&mut ctx, completion);

    for file in opt.files.iter() {
        if !file.is_file() {
            continue;
        }

        editor.open_file(file);
    }

    // Register the event handler on file updates.
    let (file_updated_tx, file_updated_rx) = unbounded_channel::<Arc<RwLock<Buffer>>>();
    tokio::spawn(on_file_change(
        file_updated_rx,
        editor.workspace_dir().to_path_buf(),
        editor.syncd().clone(),
    ));

    let backup_dir = backup_dir();
    let mut updated = false;
    while !editor.exited() {
        let mut ctx = Context {
            editor: &mut editor,
            event_tx: &event_tx,
        };

        if updated {
            compositor.render_to_terminal(&mut ctx);
        }

        match timeout(Duration::from_millis(400), event_rx.recv()).await {
            Ok(Some(ev)) => {
                let started_at = Instant::now();
                let prev_ver = editor.current_buffer().read().id_and_version();

                let mut ctx = Context {
                    editor: &mut editor,
                    event_tx: &event_tx,
                };
                compositor.handle_event(&mut ctx, ev);
                while let Ok(Some(ev)) = timeout(Duration::from_micros(400), event_rx.recv()).await
                {
                    compositor.handle_event(&mut ctx, ev);
                }

                let new_ver = editor.current_buffer().read().id_and_version();
                if prev_ver != new_ver {
                    // Switched or modified the current buffer.
                    file_updated_tx.send(editor.current_buffer().clone()).ok();
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
                // Timeout.
                let mut buffer = editor.current_buffer().write();
                buffer.update_backup(&backup_dir);
                buffer.mark_undo_point();
                updated = false;
            }
            Err(_) => {}
        }
    }
}

async fn on_file_change(
    mut rx: UnboundedReceiver<Arc<RwLock<Buffer>>>,
    workspace_dir: PathBuf,
    syncd: Arc<Mutex<SyncdClient>>,
) {
    while let Some(buffer_lock) = rx.recv().await {
        let (lang, file_modified_req, completion_req) = {
            let buffer = buffer_lock.read();
            let path = match buffer.path() {
                Some(path) => path,
                None => {
                    continue;
                }
            };

            // Ignore files that're not under the workspace directory.
            if !path.starts_with(&workspace_dir) {
                continue;
            }

            let lang = buffer.lang();
            let file_modified_req = LspRequest::UpdateFile {
                path: path.to_owned(),
                version: buffer.version(),
                text: buffer.text(),
            };

            let completion_req = LspRequest::Completion {
                path: path.to_owned(),
                position: buffer.main_cursor_pos(),
            };

            (lang, file_modified_req, completion_req)
        };

        if let Err(err) = syncd
            .lock()
            .await
            .call_lsp_method(lang, file_modified_req)
            .await
        {
            warn!("failed to send UpdateFile request: {}", err);
        }

        trace!("sending completion message...");
        if let Err(err) = syncd
            .lock()
            .await
            .call_lsp_method(lang, completion_req)
            .await
        {
            warn!("failed to call Completion request: {}", err);
        }
    }

    trace!("exiting file update handler");
}
