#![allow(unused)]
#![feature(test)]

extern crate test;

#[macro_use]
extern crate log;

use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;

use noa_common::{logger::install_logger, time_report::TimeReport};
use noa_compositor::{terminal::Event, Compositor};
use tokio::sync::{oneshot, Notify};
use ui::{
    bottom_line_view::BottomLineView, buffer_view::BufferView, finder_view::FinderView,
    too_small_view::TooSmallView,
};

mod clipboard;
mod document;
mod editor;
mod fuzzy;
mod highlighting;
mod movement;
mod notification;
mod ui;
mod view;
mod words;

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    let boot_time = TimeReport::new("boot time");

    install_logger("main");
    let args = Args::parse();

    let workspace_dir = args
        .files
        .iter()
        .find(|path| path.is_dir())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));

    let mut editor = editor::Editor::new();
    let mut compositor = Compositor::new();

    let mut open_finder = true;
    for path in args.files {
        if !path.is_dir() {
            editor.open_file(&path);
            open_finder = false;
        }
    }

    let (quit_tx, mut quit_rx) = oneshot::channel();
    let render_request = Arc::new(Notify::new());
    compositor.add_frontmost_layer(Box::new(TooSmallView::new("too small!")));
    compositor.add_frontmost_layer(Box::new(BufferView::new(quit_tx)));
    compositor.add_frontmost_layer(Box::new(BottomLineView::new()));
    compositor.add_frontmost_layer(Box::new(FinderView::new(
        render_request.clone(),
        &workspace_dir,
    )));

    if open_finder {
        compositor
            .get_mut_surface_by_name::<FinderView>("finder")
            .set_active(true);
    }

    compositor.render_to_terminal(&mut editor);
    drop(boot_time);

    let mut idle_timer = tokio::time::interval(Duration::from_millis(1200));
    loop {
        let mut skip_rendering = false;
        tokio::select! {
            biased;

            _ = &mut quit_rx => {
                break;
            }

            Some(ev) = compositor.recv_terminal_event() => {
                let _event_tick_time = Some(TimeReport::new("event tick"));
                match ev {
                    Event::Input(input) => {
                        compositor.handle_input(&mut editor, input);
                    }
                    Event::Resize { height, width } => {
                        compositor.resize_screen(height, width);
                    }
                }
            }

            _ = render_request.notified() => {
            }

            _ = idle_timer.tick()  => {
                editor.documents.current_mut().idle_job();
                skip_rendering = true;
            }
        }

        if !skip_rendering {
            compositor.render_to_terminal(&mut editor);
        }
        idle_timer.reset();
    }
}
