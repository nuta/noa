#![allow(unused)]

use crate::{buffer_set::OpenedFile, sync_client::SyncClient, theme::DEFAULT_THEME};
use anyhow::Result;
use buffer::BufferSurface;
use buffer_set::BufferSet;
use completion::CompletionSurface;
use git::Repo;
use minimap::{MiniMap, MiniMapCategory};
use noa_buffer::{BufferId, Point};
use noa_common::{
    dirs::{backup_dir, noa_bin_args},
    logger::install_logger,
    oops::OopsExt,
    sync_protocol::{FileLocation, LspRequest, Notification},
    tmux,
};
use noa_cui::Compositor;
use parking_lot::RwLock;
use std::{
    env::current_dir,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use structopt::StructOpt;
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedSender},
        Mutex,
    },
    time::timeout,
};

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

mod actions;
mod buffer;
mod buffer_set;
mod completion;
mod finder;
mod fuzzy_set;
mod git;
mod minimap;
mod selector;
mod sync_client;
mod theme;
mod view;

#[derive(StructOpt)]
struct Opt {
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
    #[structopt(long)]
    lineno: Option<usize>,
    #[structopt(long)]
    column: Option<usize>,
    #[structopt(long)]
    open_path_in_tmux: bool,
    #[structopt(long)]
    tmux_pane: Option<String>,
    #[structopt(long)]
    tmux_mouse_y: Option<usize>,
    #[structopt(long)]
    tmux_mouse_x: Option<usize>,
}

async fn open_path_in_tmux(opt: Opt) -> Result<()> {
    let (path, pos) = tmux::resolve_path_on_cursor(
        opt.tmux_pane.expect("--tmux-pane is required").as_str(),
        opt.tmux_mouse_y.expect("--tmux-mouse-y is required"),
        opt.tmux_mouse_x.expect("--tmux-mouse-x is required"),
    )?;

    let (tx, _) = unbounded_channel();
    let mut sync = SyncClient::new(Path::new("/"), tx);

    match tmux::get_other_noa_pane_id() {
        Ok(pane_id) => {
            sync.call_buffer_open_file_in_other(&pane_id, &path, Some(pos))
                .await?;
        }
        Err(err) => {
            trace!(
                "failed to open in other panes, spawning a new noa: error={:?}",
                err
            );
            std::process::Command::new("tmux")
                .args(&["splitw", "-h"])
                .args(noa_bin_args())
                .arg("--lineno")
                .arg(format!("{}", pos.y))
                .arg("--column")
                .arg(format!("{}", pos.x))
                .arg(path)
                .spawn()?
                .wait()?;
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum Event {
    ReDraw,
    OpenFile(FileLocation),
    Notification(Notification),
}

#[tokio::main]
async fn main() {
    install_logger("main");
    let opt = Opt::from_args();

    if opt.open_path_in_tmux {
        if let Err(err) = open_path_in_tmux(opt).await {
            error!("failed to open a path in tmux: {:?}", err);
        }
        return;
    }

    let workspace_dir = match opt.files.get(0) {
        Some(file_or_dir) if file_or_dir.is_dir() => file_or_dir.clone(),
        _ => current_dir().unwrap(),
    };

    // Initialize editor components.
    let (event_tx, mut event_rx) = unbounded_channel();
    let (noti_tx, mut noti_rx) = unbounded_channel();
    let mut buffers = BufferSet::new();
    let minimap = Arc::new(parking_lot::Mutex::new(MiniMap::new()));
    let repo = Arc::new(Repo::open(&workspace_dir).ok());
    let sync = Arc::new(Mutex::new(SyncClient::new(&workspace_dir, noti_tx)));
    let theme = DEFAULT_THEME;

    let mut cursor_pos = opt.lineno.map(|lineno| {
        Point::new(
            lineno.saturating_sub(1),
            opt.column
                .map(|column| column.saturating_sub(1))
                .unwrap_or(0),
        )
    });

    // Open speicifed file or the workspace dir.
    for file in opt.files.iter() {
        if !file.is_file() {
            continue;
        }

        // TODO: report error
        buffers.open_file(file, cursor_pos.take());
    }

    // Initialize UI.
    let buffers = Arc::new(RwLock::new(buffers));
    let mut compositor = Compositor::new();
    let completion = CompletionSurface::new(buffers.clone(), event_tx.clone(), sync.clone());
    let buffer = BufferSurface::new(
        theme,
        buffers.clone(),
        &workspace_dir,
        event_tx.clone(),
        sync.clone(),
        minimap.clone(),
    );
    compositor.push_layer(buffer);
    compositor.push_layer(completion);

    // Handle notifications.
    {
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            while let Some(noti) = noti_rx.recv().await {
                event_tx.send(Event::Notification(noti)).ok();
            }
        });
    }

    let before_event = || {
        // The buffer version *before* we handle user inputs that possibly modifies
        // the buffer.
        let started_at = Instant::now();
        let prev = buffers.read().current_file().read().buffer.id_and_version();
        (started_at, prev)
    };

    let after_event = |(started_at, prev): (Instant, (BufferId, usize))| {
        let buffers = buffers.read();
        let current_file = buffers.current_file();
        let f = current_file.read();
        if prev != f.buffer.id_and_version() {
            // Switched or modified the current buffer.
            // Update the syntax highlighting.
            {
                let event_tx = event_tx.clone();
                let current_file = current_file.clone();
                tokio::spawn(async move {
                    let (rope, mut parser) = {
                        let f = current_file.read();
                        let rope = f.buffer.rope().clone();
                        let parser = match f.buffer.lang().syntax_highlighting_parser() {
                            Some(parser) => parser,
                            None => return,
                        };
                        (rope, parser)
                    };

                    if let Some(tree) = parser.parse(rope.text(), None) {
                        current_file.write().syntax_highlight = Some(tree);
                    }

                    event_tx.send(Event::ReDraw).ok();
                });
            }

            // Sync updated file contents with LSP.
            {
                let sync = sync.clone();
                let current_file = current_file.clone();
                tokio::spawn(async move {
                    sync.lock()
                        .await
                        .call_lsp_method_for_file(&current_file, |path, f| LspRequest::UpdateFile {
                            path,
                            text: f.buffer.text(),
                            version: f.buffer.version(),
                        })
                        .await
                        .oops();
                });
            }

            // Sync updated file contents with buffer-sync.
            {
                let sync = sync.clone();
                let current_file = current_file.clone();
                tokio::spawn(async move {
                    let (path, text) = {
                        let f = current_file.read();
                        let path = match f.buffer.path() {
                            Some(path) => path.to_owned(),
                            None => return,
                        };

                        (path, f.buffer.text())
                    };

                    sync.lock()
                        .await
                        .call_buffer_update_file(&path, text)
                        .await
                        .oops();
                });
            }

            trace!(
                "event handling took {} us",
                started_at.elapsed().as_micros()
            );
        }
    };

    let on_idle = {
        let repo = repo.clone();
        let buffers = buffers.clone();
        let backup_dir = backup_dir();
        move || {
            let buffers = buffers.read();
            let backup_dir = backup_dir.clone();

            // Update git line statuses.
            let current_file = buffers.current_file().clone();
            let minimap = minimap.clone();
            let repo = repo.clone();
            tokio::spawn(async move {
                if let Some(repo) = repo.as_ref() {
                    let current_file = current_file.clone();
                    let (path, snapshot) = {
                        let mut f = current_file.read();
                        let path = f.buffer.path().map(Path::to_path_buf);
                        let snapshot = f.buffer.take_snapshot();
                        (path, snapshot)
                    };

                    if let Some(path) = path {
                        minimap
                            .lock()
                            .update_git_line_statuses(&repo, &path, snapshot.text());
                    }
                }
            });

            // Create a backup file and add a undo point.
            let current_file = buffers.current_file().clone();
            tokio::spawn(async move {
                let mut f = current_file.write();
                f.buffer.update_backup(&backup_dir);
                f.buffer.mark_undo_point();
            });
        }
    };

    let cursor_pos = {
        let buffers = buffers.clone();
        move || {
            let pos = buffers
                .read()
                .current_file()
                .read()
                .buffer
                .main_cursor_pos();

            // FIXME: Convert into display (y, x)
            (pos.y, pos.x)
        }
    };

    compositor
        .mainloop(before_event, after_event, on_idle, cursor_pos)
        .await;
}
