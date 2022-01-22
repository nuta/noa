#![allow(unused)]
#![feature(test)]

extern crate test;

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

use std::path::PathBuf;

use clap::Parser;

use noa_common::{logger::install_logger, time_report::TimeReport};
use noa_compositor::{terminal::Event, Compositor};
use tokio::{sync::oneshot, time::Instant};
use ui::{buffer_view::BufferView, finder_view::FinderView, too_small_view::TooSmallView};

mod clipboard;
mod document;
mod editor;
mod fuzzy;
mod highlighting;
mod notification;
mod path;
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

    let workspace_dir = PathBuf::from(".");

    let mut editor = editor::Editor::new();
    let mut compositor = Compositor::new();

    let (quit_tx, mut quit) = oneshot::channel();
    compositor.add_frontmost_layer(Box::new(TooSmallView::new("too small!")));
    compositor.add_frontmost_layer(Box::new(BufferView::new(quit_tx)));
    compositor.add_frontmost_layer(Box::new(FinderView::new(&workspace_dir)));

    compositor.render_to_terminal(&mut editor);
    drop(boot_time);

    let mut started_at = Instant::now();
    loop {
        trace!("event tick = {}ms", started_at.elapsed().as_millis());

        tokio::select! {
            biased;

            _ = &mut quit => {
                break;
            }

            Some(ev) = compositor.recv_terminal_event() => {
                started_at = Instant::now();
                match ev {
                    Event::Input(input) => {
                        compositor.handle_input(&mut editor, input);
                    }
                    Event::Resize { height, width } => {
                        compositor.resize_screen(height, width);
                    }
                }
            }
        }

        compositor.render_to_terminal(&mut editor);
    }
}
