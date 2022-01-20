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

use noa_common::logger::install_logger;
use noa_compositor::{terminal::Event, Compositor};
use tokio::sync::oneshot;
use ui::buffer_view::BufferView;

mod clipboard;
mod document;
mod editor;
mod highlighting;
mod notification;
mod ui;
mod view;

#[derive(Parser, Debug)]
struct Args {
    #[clap(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    install_logger("main");
    let args = Args::parse();

    let mut editor = editor::Editor::new();
    let mut compositor = Compositor::new();

    let (quit_tx, mut quit) = oneshot::channel();
    compositor.add_frontmost_layer(Box::new(BufferView::new(quit_tx)), true, 0, 0);

    loop {
        compositor.render_to_terminal();

        tokio::select! {
            biased;

            _ = &mut quit => {
                break;
            }

            Some(ev) = compositor.recv_terminal_event() => {
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
    }
}
